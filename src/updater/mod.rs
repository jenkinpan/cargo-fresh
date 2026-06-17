use anyhow::Result;
use colored::*;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::process::Output;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::display::{pb_status_dim, pb_status_err, pb_status_warn, status, status_dim};
use crate::downloader::{
    self,
    events::{DownloaderError, ProgressEvent},
    InstallSpec,
};
use crate::locale::detection::detect_language;
use crate::models::{
    InstallMethod, InstallOpts, PackageSource, UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_TICK_MS,
    RETRY_DELAY_MS, VERSION_UPDATE_DELAY_MS,
};
use crate::package::{get_installed_version, invalidate_installed_version};

/// 全局共享的 `MultiProgress` —— 0.11.0 串行只挂一条 bar, 0.12.0 并发调度器
/// 复用同一个实例同时挂 N 条。`pb.println`/`mp.println` 会在所有 bar 上方
/// 滚屏, 这样状态行 ("Updating ripgrep ..." 等) 不会被 bar 覆盖。
pub fn multi_progress() -> &'static MultiProgress {
    static MP: OnceLock<MultiProgress> = OnceLock::new();
    MP.get_or_init(|| {
        use std::io::IsTerminal;
        let mp = MultiProgress::new();
        if !std::io::stderr().is_terminal() || crate::display::is_json_mode() {
            mp.set_draw_target(ProgressDrawTarget::hidden());
        }
        mp
    })
}

/// rustup-style 下载条样式 —— 名字右对齐到统一宽度, 后接 bar + bytes + speed + ETA。
fn download_bar_style(name_width: usize) -> ProgressStyle {
    ProgressStyle::with_template(&format!(
        "{{msg:>{name_width}.cyan}} [{{bar:24.cyan/blue}}] {{bytes:>8}}/{{total_bytes:>8}} ({{percent:>3}}%) {{binary_bytes_per_sec:>10}} ETA {{eta:>3}}"
    ))
    .unwrap()
    .progress_chars("##-")
}

/// 非下载阶段 spinner 样式 —— 名字右对齐, 后接 spinner + 状态动词。
fn spinner_style(name_width: usize) -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template(&format!(
            "{{msg:>{name_width}.cyan}} {{spinner:.green}} {{prefix}}"
        ))
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
}

/// 终态静态行: 不带 spinner/bar, 名字右对齐 + 动词 + 详情。
fn static_style(name_width: usize) -> ProgressStyle {
    ProgressStyle::with_template(&format!("{{msg:>{name_width}.cyan}} {{prefix}}")).unwrap()
}

/// 一组同时显示的包行的视图. main 在更新循环前调用 `Plan::new(names)`,
/// 然后每个包用 `plan.row(i)` 拿到 PackageRow, 传给 update_package.
/// 0.12.0 并发调度器同样用这个——只需把循环 spawn 起来。
pub struct UpdatePlan {
    rows: Vec<ProgressBar>,
    name_width: usize,
}

impl UpdatePlan {
    pub fn new(names: &[String]) -> Self {
        let name_width = names
            .iter()
            .map(|n| n.chars().count())
            .max()
            .unwrap_or(0)
            .max(10);
        let mp = multi_progress();
        let rows: Vec<ProgressBar> = names
            .iter()
            .map(|n| {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(static_style(name_width));
                pb.set_message(n.clone());
                pb.set_prefix("pending".dimmed().to_string());
                pb
            })
            .collect();
        Self { rows, name_width }
    }
    pub fn row(&self, i: usize) -> ProgressBar {
        self.rows[i].clone()
    }
    pub fn name_width(&self) -> usize {
        self.name_width
    }
}

/// 创建当前正在更新的包的 progress bar (初始 spinner 形态)。
///
/// 挂在全局 `MultiProgress` 上 —— 0.11.0 一次只有一条, 但 0.12.0 并发调度器
/// 会同时挂多条; 同一种创建路径方便复用。
///
/// 必须配合 [`PbGuard`] 使用——guard drop 时 `finish_and_clear()` 把行
/// 从 MultiProgress 里摘掉, 残留不会污染输出。
///
/// 非 TTY (CI / 管道 / `tee`) 下 MultiProgress 整体走 hidden draw target——
/// 单条 bar 的样式仍然有效, 但不会向终端写帧, `pb.println` 走标准 eprintln 路径。
/// 旧入口 (仍被旧测试/旧路径调用)。新 main 路径走 UpdatePlan + activate_row。
pub fn create_progress_bar(package_name: &str) -> ProgressBar {
    let pb = multi_progress().add(ProgressBar::new_spinner());
    let nw = package_name.chars().count().max(10);
    pb.set_style(spinner_style(nw));
    pb.set_message(package_name.to_string());
    pb.set_prefix("pending".dimmed().to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));
    pb
}

/// 把"pending"行点亮成 spinner 形态 (开 steady_tick), 设置阶段动词。
pub fn activate_row(pb: &ProgressBar, name_width: usize, verb: &str) {
    pb.set_style(spinner_style(name_width));
    pb.set_prefix(verb.green().bold().to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));
}

/// 切到 rustup 风格的下载条形态.
pub fn switch_to_download_bar(pb: &ProgressBar, name_width: usize, total: u64) {
    pb.disable_steady_tick();
    pb.set_style(download_bar_style(name_width));
    pb.set_length(total);
}

/// 下载结束后切回 spinner. 保留 length/position —— finalize 阶段会用它
/// 算下载尺寸 (显示为 "installed 4.21 MiB")。
pub fn switch_to_spinner_phase(pb: &ProgressBar, name_width: usize, verb: &str) {
    pb.set_style(spinner_style(name_width));
    pb.set_prefix(verb.green().bold().to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));
}

/// 把行定格成静态"installed"行 (绿)。会调 finish, 之后 pb 不再重画。
/// 若 pb 持有下载尺寸 (length > 0, 来自 downloader 路径), 显示为 "installed X.XX MiB";
/// 否则 (cargo install 编译路径) 只显示 "installed"。
pub fn finalize_installed(pb: &ProgressBar, name_width: usize) {
    pb.disable_steady_tick();
    pb.set_style(static_style(name_width));
    let total = pb.length().unwrap_or(0);
    let prefix = if total > 0 {
        format!(
            "{} {:>10.2} MiB",
            "installed".green().bold(),
            total as f64 / 1_048_576.0
        )
    } else {
        "installed".green().bold().to_string()
    };
    pb.set_prefix(prefix);
    pb.finish();
}

/// 把行定格成静态"failed"行 (红)。
pub fn finalize_failed(pb: &ProgressBar, name_width: usize, detail: &str) {
    pb.disable_steady_tick();
    pb.set_style(static_style(name_width));
    pb.set_prefix(format!("{} {}", "failed".red().bold(), detail.dimmed()));
    pb.finish();
}

/// 把行定格成 aborted (黄).
pub fn finalize_aborted(pb: &ProgressBar, name_width: usize) {
    pb.disable_steady_tick();
    pb.set_style(static_style(name_width));
    pb.set_prefix("aborted".yellow().bold().to_string());
    pb.finish();
}

/// 长跑包的告警阈值——超过这个秒数 spawn 一条 `Slow` 提示，
/// 帮助用户在 18 个包升级里看出"卡住的那个是谁"。
const SLOW_PACKAGE_THRESHOLD_SECS: u64 = 30;

/// 起一个任务，到点（默认 30s）后在 `pb` 上打 `Slow <name> running for Ns ...`。
/// 调用方 `Drop` 返回的 handle 即可取消这条提示（包正常结束时立即生效）。
fn spawn_slow_warning(pb: ProgressBar, package_name: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(SLOW_PACKAGE_THRESHOLD_SECS)).await;
        let elapsed = pb.elapsed().as_secs();
        pb_status_dim(
            &pb,
            "Slow",
            &format!(
                "{} running for {}s (likely building from source)",
                package_name.cyan(),
                elapsed
            ),
        );
    })
}

/// Drop 守卫：保证 spinner 在 `update_package` 任何返回路径都被 `finish_and_clear`。
///
/// 旧版本依赖手动调用，多个 return 分支容易漏写，导致 spinner 帧残留在
/// 用户终端（已报告的"转圈进度残留"bug 的根因之一）。
struct PbGuard<'a>(&'a ProgressBar);

impl Drop for PbGuard<'_> {
    fn drop(&mut self) {
        self.0.finish_and_clear();
    }
}

/// 中止 `spawn_slow_warning` 的 watchdog——包在阈值前完成时，避免
/// 在主流程结束后才弹出迟到的 Slow 提示。
struct SlowGuard(tokio::task::JoinHandle<()>);

impl Drop for SlowGuard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// 根据来源类型构造 cargo 子命令参数，并追加 features 相关 flags。
///
/// - `Crates`：`install --force <pkg> [--version V]`（binstall subprocess 已移除）
/// - `Git`：`install --git URL [--rev REV] --force <pkg>`
/// - `Path`：`install --path DIR --force <pkg>`
/// - `opts`：如果提供且非默认，则在基础参数后追加 `--no-default-features` / `--all-features` / `--features a,b`
fn build_args(
    package_name: &str,
    version: Option<&str>,
    source: &PackageSource,
    opts: Option<&InstallOpts>,
) -> Vec<String> {
    let mut args: Vec<String> = match source {
        PackageSource::Crates => {
            let mut a: Vec<String> = vec!["install".into()];
            a.push("--force".into());
            a.push(package_name.into());
            if let Some(v) = version {
                a.push("--version".into());
                a.push(v.into());
            }
            a
        }
        PackageSource::Git { url, rev } => {
            let mut a: Vec<String> = vec!["install".into(), "--git".into(), url.clone()];
            if let Some(r) = rev {
                a.push("--rev".into());
                a.push(r.clone());
            }
            a.push("--force".into());
            a.push(package_name.into());
            a
        }
        PackageSource::Path { dir } => vec![
            "install".into(),
            "--path".into(),
            dir.clone(),
            "--force".into(),
            package_name.into(),
        ],
        // Unknown 来源不应到这一步——check_package_updates 会跳过它。
        // 万一到了，给个明显错的命令让上层报错而不是默默 cargo install。
        PackageSource::Unknown(raw) => vec![
            "install".into(),
            "--unknown-source-marker".into(),
            raw.clone(),
            package_name.into(),
        ],
    };

    // 追加 features 选项（Unknown 源不追加——它本就要让上层报错）。
    if let (Some(o), false) = (opts, matches!(source, PackageSource::Unknown(_))) {
        if o.no_default_features {
            args.push("--no-default-features".into());
        }
        if o.all_features {
            args.push("--all-features".into());
        }
        if !o.features.is_empty() {
            args.push("--features".into());
            args.push(o.features.join(","));
        }
    }
    args
}

async fn run_cargo(pb: &ProgressBar, args: &[String]) -> Result<Output> {
    pb_status_dim(pb, "Running", &format!("cargo {}", args.join(" ")));
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));
    let output = Command::new("cargo")
        .args(args)
        // stdin 接 /dev/null:cargo-fresh 已替用户做过确认,任何 cargo 子命令
        // 都不该再交互。即便哪天某个命令仍想提示,继承 /dev/null 会让它立刻
        // 读到 EOF、走默认或快速失败,而不是顶着被管道吞掉的提示符无声挂死。
        // 这是 --no-confirm 之外的第二层防线(防同类 bug 从别的路径复发)。
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await?;
    pb.disable_steady_tick();
    Ok(output)
}

/// 命令执行成功后，确认安装版本并打印对应文案，返回 UpdateResult。
///
/// `new_version: None` 表示命令成功但无法读到安装后的版本（例：cargo install --list 失败）。
/// 调用方可据此决定是否重试。
async fn verify_and_report_update(
    pb: &ProgressBar,
    package_name: &str,
    old_version: &Option<String>,
) -> UpdateResult {
    let language = detect_language();
    // 升级成功后让该包的版本缓存失效，下次 get_installed_version 重新查 cargo
    invalidate_installed_version(package_name);

    tokio::time::sleep(tokio::time::Duration::from_millis(VERSION_UPDATE_DELAY_MS)).await;

    match get_installed_version(package_name).await {
        Ok(Some(new_version)) if old_version.as_ref() != Some(&new_version) => {
            // 不再滚屏打 "Updated X 旧 -> 新" —— rustup 风格的行末态 (finalize_installed)
            // 和末尾 summary 已经各报一次, 这里再来一遍是冗余。
            let _ = (pb, language); // pb 仍由调用方持有为后续 finalize 用
            UpdateResult::new(
                package_name.to_string(),
                old_version.clone(),
                Some(new_version),
                true,
            )
        }
        Ok(Some(_)) => {
            pb_status_warn(
                pb,
                "Unchanged",
                &format!(
                    "{} {}",
                    package_name.cyan(),
                    language.get_text("version_unchanged").dimmed()
                ),
            );
            UpdateResult::new(
                package_name.to_string(),
                old_version.clone(),
                old_version.clone(),
                true,
            )
        }
        _ => {
            pb_status_warn(
                pb,
                "Warning",
                &format!(
                    "{} {}",
                    package_name.cyan(),
                    detect_language()
                        .get_text("package_update_verification_failed")
                        .replace("{} ", "")
                        .dimmed()
                ),
            );
            UpdateResult::new(package_name.to_string(), old_version.clone(), None, true)
        }
    }
}

fn report_command_failure(pb: &ProgressBar, package_name: &str, output: &Output) {
    pb_status_err(
        pb,
        "Failed",
        &format!(
            "{} (exit code: {})",
            package_name.red(),
            output.status.code().unwrap_or(-1)
        ),
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        pb_status_dim(pb, "stderr", &format!("{}", stderr.trim().dimmed()));
    }
}

/// 尝试用自托管 downloader 安装包。
///
/// 返回值三分支：
/// - `Ok(true)`  — 安装成功，调用方直接走 verify_and_report_update。
/// - `Ok(false)` — 不支持或失败，调用方应回退到 cargo install。
/// - `Err(_)`    — Cancelled（Ctrl-C），调用方应立即返回 Ok(None)。
async fn try_downloader_install(
    pb: &ProgressBar,
    name_width: usize,
    package_name: &str,
    version: &str,
    old_version: &Option<String>,
    cancel_arc: Arc<AtomicBool>,
    verbose: bool,
) -> Result<bool, DownloaderError> {
    // 先从 crates.io API 拿 repo_url；拿不到直接走 cargo install。
    // HTTP 客户端建不起来 → downloader 没法工作，当作"不支持"回退 cargo install。
    let Ok(client) = crate::package::http_client() else {
        return Ok(false);
    };
    let repo_url = crate::package::crates_api::fetch_repo_url(client, package_name).await;
    crate::display::status_debug(
        "downloader",
        &format!(
            "{package_name}: repo_url={}",
            repo_url.as_deref().unwrap_or("<none>")
        ),
    );
    if repo_url.is_none() {
        pb_status_dim(
            pb,
            "Downloader",
            &format!("{package_name}: no repo URL, falling back to cargo install"),
        );
        return Ok(false);
    }

    // 从 .crates2.json 查 bins[] —— ripgrep 包名 vs "rg" binary 名要靠这个区分
    let bins = crate::package::registry::cargo_home()
        .map(|home| crate::package::crates2::lookup_bins(&home, package_name))
        .unwrap_or_default();
    crate::display::status_debug("downloader", &format!("{package_name}: bins={bins:?}"));

    let (tx, mut rx) = mpsc::unbounded_channel::<ProgressEvent>();
    let spec = InstallSpec {
        name: package_name.to_string(),
        version: version.to_string(),
        repo_url,
        bins,
    };

    // 消费进度事件 —— 行已经在 main.rs 预注册, 这里只切样式 + 更新 prefix
    let pb_clone = pb.clone();
    let verbose_flag = verbose;
    let event_handle = tokio::spawn(async move {
        let mut bar_initialized = false;
        while let Some(event) = rx.recv().await {
            match event {
                ProgressEvent::Resolving { .. } => {
                    activate_row(&pb_clone, name_width, "resolving");
                }
                ProgressEvent::UrlCandidate { url, .. } => {
                    if verbose_flag {
                        pb_status_dim(&pb_clone, "Trying", &url.dimmed().to_string());
                    }
                }
                ProgressEvent::Downloading { got, total, .. } => {
                    match (bar_initialized, total) {
                        (false, Some(t)) if t > 0 => {
                            switch_to_download_bar(&pb_clone, name_width, t);
                            pb_clone.set_position(got);
                            bar_initialized = true;
                        }
                        (true, _) => {
                            pb_clone.set_position(got);
                        }
                        _ => {
                            // 无 content-length —— 留在 spinner 上, 仅更新 prefix
                            pb_clone.set_prefix(format!(
                                "{} {:.1} MiB",
                                "downloading".green().bold(),
                                got as f64 / 1_048_576.0
                            ));
                        }
                    }
                }
                ProgressEvent::Verifying { .. } => {
                    if bar_initialized {
                        switch_to_spinner_phase(&pb_clone, name_width, "verifying");
                        bar_initialized = false;
                    } else {
                        pb_clone.set_prefix("verifying".green().bold().to_string());
                    }
                }
                ProgressEvent::Extracting { .. } => {
                    if bar_initialized {
                        switch_to_spinner_phase(&pb_clone, name_width, "extracting");
                        bar_initialized = false;
                    } else {
                        pb_clone.set_prefix("extracting".green().bold().to_string());
                    }
                }
                ProgressEvent::Installing { .. } => {
                    pb_clone.set_prefix("installing".green().bold().to_string());
                }
                ProgressEvent::Done { .. } | ProgressEvent::Failed { .. } => {
                    // handled by caller
                }
            }
        }
    });

    let result =
        downloader::download_and_install(client, spec, old_version.clone(), tx, cancel_arc).await;

    // 等事件消费者结束
    let _ = event_handle.await;

    match result {
        Ok(_outcome) => Ok(true),
        Err(DownloaderError::Cancelled) => Err(DownloaderError::Cancelled),
        Err(DownloaderError::Unsupported(reason)) => {
            pb_status_dim(
                pb,
                "Downloader",
                &format!("{package_name}: unsupported ({reason:?}), falling back to cargo install"),
            );
            Ok(false)
        }
        Err(DownloaderError::Failed { kind, source }) => {
            pb_status_dim(
                pb,
                "Fallback",
                &format!(
                    "{package_name}: downloader failed ({kind:?}: {source}), falling back to cargo install"
                ),
            );
            Ok(false)
        }
    }
}

/// 更新单个包。
///
/// 返回 `Ok(None)` 表示**用户按 Ctrl-C 中途取消了这个包**——它既不是成功
/// 也不是失败,调用方应据此停止后续包并标记中止,不要把它计入失败数。
/// `cancel` 是 `main` 持有的 `Arc<AtomicBool>`，Ctrl-C 信号处理任务置位后
/// 下载器和 cargo 子进程循环都能实时感知。
// 参数已到 8 个(本就贴着 clippy 阈值)。这些是"每包参数 + 全程运行上下文"
// 的混合，真要收拢应抽 UpdateContext 结构体——留作独立重构。
#[allow(clippy::too_many_arguments)]
pub async fn update_package(
    package_name: &str,
    target_version: Option<&str>,
    source: &PackageSource,
    install_opts: Option<&InstallOpts>,
    dry_run: bool,
    verbose: bool,
    cancel: Arc<AtomicBool>,
    row: Option<(ProgressBar, usize)>,
) -> Result<Option<UpdateResult>> {
    // 在做任何事(连 cargo install --list 都还没查)之前先看取消标志。
    if cancel.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let language = detect_language();
    let old_version = get_installed_version(package_name).await.ok().flatten();

    // 路径策略：
    // - Crates 源 + 默认 features：自托管 downloader 为主路径（0.11+）；
    //   Unsupported/Failed 时回退 cargo install。cargo binstall subprocess 不再调用。
    // - Crates 源 + 自定义 features：直接走 cargo install（downloader 不支持任意 features）。
    // - Git / Path 源：cargo install（downloader 仅支持 crates.io 包）。
    let opts_allow_downloader = install_opts.is_none_or(|o| o.is_default());

    crate::display::status_debug(
        "updater",
        &format!(
            "{package_name}: source={} default_features={} → {}",
            source.kind_str(),
            opts_allow_downloader,
            if matches!(source, PackageSource::Crates) && opts_allow_downloader {
                "downloader (fallback: cargo install)"
            } else {
                "cargo install"
            }
        ),
    );

    if verbose {
        if install_opts.is_none() && matches!(source, PackageSource::Crates) {
            status_dim(
                "Check",
                &format!("{package_name}: no install metadata, using default features"),
            );
        } else if !opts_allow_downloader && matches!(source, PackageSource::Crates) {
            status_dim(
                "Check",
                &format!("{package_name}: custom features, skipping downloader"),
            );
        }
    }

    // cargo install args — used for fallback (Crates) or primary (Git/Path/custom-features).
    let cargo_install_args = build_args(package_name, target_version, source, install_opts);

    // dry-run：直接打印到 stdout（绕过 progress bar 避免 finish 时被清掉），
    // 立即返回成功结果，不调用 cargo。
    if dry_run {
        let marker = source.marker();
        let header = if marker.is_empty() {
            package_name.cyan().bold().to_string()
        } else {
            format!("{} {}", package_name.cyan().bold(), marker.dimmed())
        };
        // For Crates + default features, show the downloader as primary path.
        if matches!(source, PackageSource::Crates) && opts_allow_downloader {
            status(
                "Would run",
                &format!(
                    "{header}: self-hosted downloader → cargo {}",
                    cargo_install_args.join(" ")
                ),
            );
        } else {
            status(
                "Would run",
                &format!("{}: cargo {}", header, cargo_install_args.join(" ")),
            );
        }
        return Ok(Some(UpdateResult::new(
            package_name.to_string(),
            old_version.clone(),
            old_version,
            true,
        )));
    }

    // row: 来自 main 的 UpdatePlan 预注册行 + 对齐宽度。
    // 旧路径 (没传 row, 比如老测试) 仍走 create_progress_bar 自建 spinner。
    let (pb, name_width) = match row {
        Some((pb, w)) => {
            activate_row(&pb, w, "starting");
            (pb, w)
        }
        None => {
            let pb = create_progress_bar(package_name);
            let w = package_name.chars().count().max(10);
            (pb, w)
        }
    };
    let _pb_guard = PbGuard(&pb);
    let slow_handle = spawn_slow_warning(pb.clone(), package_name.to_string());
    let _slow_guard = SlowGuard(slow_handle);

    match source {
        PackageSource::Crates | PackageSource::Git { .. } | PackageSource::Path { .. } => {
            // 安装路径 (downloader vs cargo install) 不再逐包播报——
            // 在 print_update_summary 末尾按方法分组汇报, 减少滚屏噪音
        }
        PackageSource::Unknown(_) => {
            // 不应该到这一步——main 不会把 unknown 包放进 selections。
            pb_status_warn(
                &pb,
                "Skip",
                &format!("{} {}", package_name, source.marker()),
            );
            return Ok(Some(UpdateResult::new(
                package_name.to_string(),
                old_version,
                None,
                false,
            )));
        }
    }

    // 自托管 downloader 路径：Crates 源 + 默认 features。
    // 无论系统是否安装了 cargo-binstall，都走这条路。
    // Unsupported/Failed → 回退 cargo install。Cancelled → 中止。
    if matches!(source, PackageSource::Crates) && opts_allow_downloader {
        if let Some(v) = target_version {
            match try_downloader_install(
                &pb,
                name_width,
                package_name,
                v,
                &old_version,
                cancel.clone(),
                verbose,
            )
            .await
            {
                Ok(true) => {
                    let result = verify_and_report_update(&pb, package_name, &old_version)
                        .await
                        .with_install_method(InstallMethod::Downloader);
                    return Ok(Some(result));
                }
                Ok(false) => {
                    // Unsupported or failed — fall through to cargo install subprocess.
                    // 不打 Using 行 —— 末尾 summary 按方法分组通报。
                }
                Err(DownloaderError::Cancelled) => {
                    pb_status_warn(&pb, "Aborted", &package_name.cyan().to_string());
                    return Ok(None);
                }
                Err(_) => unreachable!("try_downloader_install only returns Cancelled as Err"),
            }
        }
    }

    // cargo install subprocess — fallback for Crates (after downloader fail/unsupported),
    // or primary for Git/Path/custom-features. 这条路径肯定是源码编译, 让行
    // 上的 phase 文案直接说 "compiling from source"——用户看到 ripgrep 这种
    // 走了 fallback 的就会明白接下来要慢一截 (不是卡住), 也对得上末尾 summary
    // 的 "源码编译" 分组。
    pb.disable_steady_tick();
    pb.set_length(0);
    pb.set_position(0);
    pb.set_style(spinner_style(name_width));
    pb.set_prefix("compiling from source".yellow().bold().to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        // 每次重试前检查取消标志:用户按 Ctrl-C 后立即停手,不再消耗重试
        // 次数,也不再 spawn 一个注定被同进程组 SIGINT 杀死的子进程。
        if cancel.load(Ordering::SeqCst) {
            pb_status_warn(&pb, "Aborted", &package_name.cyan().to_string());
            return Ok(None);
        }

        if attempt > 1 {
            pb.set_message(language.format_text(
                "retry_attempt",
                &[
                    ("attempt", &attempt.to_string()),
                    ("name", &package_name.cyan().to_string()),
                ],
            ));
        }

        let output = run_cargo(&pb, &cargo_install_args).await?;

        if output.status.success() {
            let result = verify_and_report_update(&pb, package_name, &old_version)
                .await
                .with_install_method(InstallMethod::CargoInstall);
            if result.new_version.is_none() && attempt < MAX_RETRY_ATTEMPTS {
                pb_status_dim(&pb, "Retry", language.get_text("waiting_retry"));
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                continue;
            }
            return Ok(Some(result));
        }

        // 命令失败。先分辨是"用户取消"还是"真实失败":Ctrl-C 会把同进程组
        // 的 cargo 子进程一起杀掉,这条命令的失败其实是被取消造成的。此时
        // 立即返回 None——不回退、不重试,否则会把一次取消放大成多次假失败。
        if cancel.load(Ordering::SeqCst) {
            pb_status_warn(&pb, "Aborted", &package_name.cyan().to_string());
            return Ok(None);
        }

        report_command_failure(&pb, package_name, &output);

        if attempt < MAX_RETRY_ATTEMPTS {
            pb_status_dim(&pb, "Retry", language.get_text("waiting_retry"));
            tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
            continue;
        }

        return Ok(Some(UpdateResult::new(
            package_name.to_string(),
            old_version,
            None,
            false,
        )));
    }

    Ok(Some(UpdateResult::new(
        package_name.to_string(),
        old_version,
        None,
        false,
    )))
}

#[cfg(test)]
mod tests {
    use super::build_args;
    use crate::models::{InstallOpts, PackageSource};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[tokio::test]
    async fn cancelled_before_start_returns_none() {
        // 复现并锁定 bug:用户按下 Ctrl-C 后,update_package 必须立即返回
        // None 表示"被取消"——绝不当成更新失败,绝不 spawn cargo 子进程。
        use std::sync::{atomic::AtomicBool, Arc};
        let cancel = Arc::new(AtomicBool::new(true));
        let result = super::update_package(
            "cargo-fresh-no-such-package",
            Some("9.9.9"),
            &PackageSource::Crates,
            None,
            false, // dry_run
            false, // verbose
            cancel,
            None, // row
        )
        .await
        .expect("update_package 不应返回 Err");
        assert!(
            result.is_none(),
            "已取消时 update_package 必须返回 None,而不是一个结果"
        );
    }

    #[test]
    fn crates_default_opts_no_extra_flags() {
        let got = build_args("ripgrep", Some("14.1.1"), &PackageSource::Crates, None);
        assert_eq!(
            got,
            s(&["install", "--force", "ripgrep", "--version", "14.1.1"])
        );
    }

    #[test]
    fn cargo_install_has_no_no_confirm() {
        // cargo install 无交互提示，也不认识 --no-confirm——绝不能加上
        let got = build_args("cargo-deny", Some("0.19.7"), &PackageSource::Crates, None);
        assert!(
            !got.contains(&"--no-confirm".to_string()),
            "cargo install 不该带 --no-confirm。实际: {got:?}"
        );
    }

    #[test]
    fn crates_with_features() {
        let opts = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["pcre2".into(), "simd".into()],
        };
        let got = build_args("ripgrep", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(
            got,
            s(&["install", "--force", "ripgrep", "--features", "pcre2,simd"])
        );
    }

    #[test]
    fn crates_no_default_and_all_features() {
        let opts = InstallOpts {
            no_default_features: true,
            all_features: true,
            features: vec![],
        };
        let got = build_args("x", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install",
                "--force",
                "x",
                "--no-default-features",
                "--all-features"
            ])
        );
    }

    #[test]
    fn git_source_with_features() {
        let opts = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["a".into()],
        };
        let src = PackageSource::Git {
            url: "https://github.com/x/y".into(),
            rev: Some("abc".into()),
        };
        let got = build_args("y", None, &src, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install",
                "--git",
                "https://github.com/x/y",
                "--rev",
                "abc",
                "--force",
                "y",
                "--features",
                "a"
            ])
        );
    }

    #[test]
    fn path_source_with_no_default_features() {
        let opts = InstallOpts {
            no_default_features: true,
            all_features: false,
            features: vec![],
        };
        let src = PackageSource::Path {
            dir: "/tmp/p".into(),
        };
        let got = build_args("p", None, &src, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install",
                "--path",
                "/tmp/p",
                "--force",
                "p",
                "--no-default-features"
            ])
        );
    }

    #[test]
    fn default_opts_some_but_empty_adds_nothing() {
        // 默认 features 不追加任何 feature flag
        let opts = InstallOpts::default();
        let got = build_args("tool", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(got, s(&["install", "--force", "tool"]));
    }

    #[test]
    fn unknown_source_ignores_opts() {
        let opts = InstallOpts {
            no_default_features: true,
            all_features: true,
            features: vec!["x".into()],
        };
        let src = PackageSource::Unknown("custom-reg".into());
        let got = build_args("tool", None, &src, Some(&opts));
        assert_eq!(
            got,
            s(&["install", "--unknown-source-marker", "custom-reg", "tool"])
        );
    }
}
