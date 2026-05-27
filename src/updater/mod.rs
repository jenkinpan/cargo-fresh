use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::process::Output;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::display::{pb_status, pb_status_dim, pb_status_err, pb_status_warn, status, status_dim};
use crate::downloader::{
    self,
    events::{DownloaderError, ProgressEvent},
    InstallSpec,
};
use crate::locale::detection::detect_language;
use crate::models::{
    InstallOpts, PackageSource, UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_TICK_MS,
    RETRY_DELAY_MS, VERSION_UPDATE_DELAY_MS,
};
use crate::package::{get_installed_version, invalidate_installed_version};

/// 创建当前正在更新的包的 spinner。
///
/// 必须配合 [`PbGuard`] 使用——guard drop 时自动调用 `finish_and_clear()`，
/// 保证 spinner 残留不会污染输出（任何提前 return 也覆盖到）。
///
/// 非 TTY（CI 日志、管道、`tee` 等）下把 draw target 设为 hidden——
/// spinner 帧在非交互终端没有意义，反而会污染日志。`pb.println` 还会
/// 正常输出，所以 pb_status* 仍然工作。
pub fn create_progress_bar(package_name: &str) -> ProgressBar {
    use std::io::IsTerminal;
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            // {elapsed} 由 indicatif 在每个 tick 重新渲染——和 enable_steady_tick
            // 一起让用户能看出哪个包正在长跑（典型场景：binstall 没有预编译，
            // 退化成本地 build 的 ripgrep / cargo-bloat 这种大 crate）
            .template("{spinner:.green} {msg} {elapsed:.dim}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(package_name.cyan().to_string());
    if !std::io::stderr().is_terminal() || crate::display::is_json_mode() {
        pb.set_draw_target(ProgressDrawTarget::hidden());
    }
    pb
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
            let mut a: Vec<String> =
                vec!["install".into(), "--git".into(), url.clone()];
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

/// 重试循环的命令选择器。
///
/// 持有主命令（Crates 源通常是 `cargo binstall`）和可选的 `cargo install`
/// 回退命令。第一次 [`switch_to_fallback`](Self::switch_to_fallback) 把活动
/// 命令一次性、永久地切到回退命令——之后每一次重试都跑 `cargo install`。
///
/// 这是修复 "binstall 失败回退到 install 后，后续重试又跑回 binstall" bug
/// 的关键：binstall 一旦在当前环境失败（典型是没有预编译产物、退化成从
/// 源码构建后仍失败），重试它只会重复那条又慢又必然失败的路径。
struct CommandSelector {
    primary: Vec<String>,
    fallback: Option<Vec<String>>,
    fell_back: bool,
}

impl CommandSelector {
    fn new(primary: Vec<String>, fallback: Option<Vec<String>>) -> Self {
        Self {
            primary,
            fallback,
            fell_back: false,
        }
    }

    /// 当前重试步应执行的 cargo 参数。
    fn current(&self) -> &[String] {
        match (self.fell_back, &self.fallback) {
            (true, Some(fb)) => fb,
            _ => &self.primary,
        }
    }

    /// 当前命令失败后调用。若存在回退命令且尚未切换，则切到回退命令并
    /// 返回 `true`（调用方据此打印 `Fallback` 状态行并立即重跑一次）。
    /// 已经回退过、或根本没有回退命令时返回 `false`。
    fn switch_to_fallback(&mut self) -> bool {
        if !self.fell_back && self.fallback.is_some() {
            self.fell_back = true;
            true
        } else {
            false
        }
    }
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
            let unknown = language.get_text("unknown_version").to_string();
            let old_str = old_version.as_ref().unwrap_or(&unknown);
            pb_status(
                pb,
                "Updated",
                &format!(
                    "{} {} -> {}",
                    package_name.cyan(),
                    old_str.red(),
                    new_version.green()
                ),
            );
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
        pb_status_dim(
            pb,
            "stderr",
            &format!("{}", stderr.trim().dimmed()),
        );
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
    package_name: &str,
    version: &str,
    old_version: &Option<String>,
    cancel_arc: Arc<AtomicBool>,
    verbose: bool,
) -> Result<bool, DownloaderError> {
    // 先从 crates.io API 拿 repo_url；拿不到直接走 cargo install
    let client = crate::package::http_client();
    let repo_url = crate::package::crates_api::fetch_repo_url(client, package_name).await;
    if repo_url.is_none() {
        pb_status_dim(
            pb,
            "Downloader",
            &format!("{package_name}: no repo URL, falling back to cargo install"),
        );
        return Ok(false);
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<ProgressEvent>();
    let spec = InstallSpec {
        name: package_name.to_string(),
        version: version.to_string(),
        repo_url,
    };

    // 消费进度事件并转成 cargo-style status 行
    let pb_clone = pb.clone();
    let pkg_name = package_name.to_string();
    let verbose_flag = verbose;
    let event_handle = tokio::spawn(async move {
        let mut last_got: u64 = 0;
        while let Some(event) = rx.recv().await {
            match event {
                ProgressEvent::Resolving { .. } => {
                    pb_status_dim(&pb_clone, "Resolving", &pkg_name.cyan().to_string());
                }
                ProgressEvent::UrlCandidate { url, .. } => {
                    if verbose_flag {
                        pb_status_dim(&pb_clone, "Trying", &url.dimmed().to_string());
                    }
                }
                ProgressEvent::Downloading { got, total, .. } => {
                    // 限流：每 256 KB 打一行，避免刷屏
                    if got.saturating_sub(last_got) >= 256 * 1024 || last_got == 0 {
                        last_got = got;
                        let msg = match total {
                            Some(t) if t > 0 => format!(
                                "{pkg_name}: {:.1} / {:.1} MB",
                                got as f64 / 1_048_576.0,
                                t as f64 / 1_048_576.0
                            ),
                            _ => format!(
                                "{pkg_name}: {:.1} MB",
                                got as f64 / 1_048_576.0
                            ),
                        };
                        pb_status_dim(&pb_clone, "Downloading", &msg);
                    }
                }
                ProgressEvent::Verifying { .. } => {
                    pb_status_dim(&pb_clone, "Verifying", &pkg_name.cyan().to_string());
                }
                ProgressEvent::Extracting { .. } => {
                    pb_status_dim(&pb_clone, "Extracting", &pkg_name.cyan().to_string());
                }
                ProgressEvent::Installing { .. } => {
                    pb_status_dim(&pb_clone, "Installing", &pkg_name.cyan().to_string());
                }
                ProgressEvent::Done { .. } | ProgressEvent::Failed { .. } => {
                    // handled by caller
                }
            }
        }
    });

    let result = downloader::download_and_install(
        client,
        spec,
        old_version.clone(),
        tx,
        cancel_arc,
    )
    .await;

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
    install_binstall: bool,
    verbose: bool,
    cancel: Arc<AtomicBool>,
) -> Result<Option<UpdateResult>> {
    // NOTE: `install_binstall` is a deprecated no-op in 0.11. The self-hosted
    // downloader replaces the cargo-binstall subprocess path. The flag is kept
    // accepted for one release to avoid breaking existing scripts.
    if install_binstall {
        static DEPRECATION_WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        DEPRECATION_WARNED.get_or_init(|| {
            status_dim(
                "Hint",
                "--install-binstall is deprecated in 0.11 and will be removed in 0.12. \
                 cargo-fresh now uses a self-hosted downloader; cargo-binstall is no longer needed.",
            );
        });
    }
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
                &format!("{header}: self-hosted downloader → cargo {}", cargo_install_args.join(" ")),
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

    let pb = create_progress_bar(package_name);
    let _pb_guard = PbGuard(&pb);
    let slow_handle = spawn_slow_warning(pb.clone(), package_name.to_string());
    let _slow_guard = SlowGuard(slow_handle);
    if let Some(ref version) = old_version {
        pb_status_dim(
            &pb,
            language.get_text("current_version_label"),
            &version.blue().to_string(),
        );
    }

    match source {
        PackageSource::Crates => {
            if opts_allow_downloader {
                pb_status_dim(&pb, "Using", language.get_text("using_binstall"));
            } else {
                pb_status_dim(&pb, "Using", language.get_text("using_install_fallback"));
            }
        }
        PackageSource::Git { .. } | PackageSource::Path { .. } => {
            pb_status_dim(
                &pb,
                "Using",
                &format!(
                    "{} {}",
                    language.get_text("using_install_fallback"),
                    source.marker().dimmed(),
                ),
            );
        }
        PackageSource::Unknown(_) => {
            // 不应该到这一步——main 不会把 unknown 包放进 selections。
            pb_status_warn(&pb, "Skip", &format!("{} {}", package_name, source.marker()));
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
            match try_downloader_install(&pb, package_name, v, &old_version, cancel.clone(), verbose).await {
                Ok(true) => {
                    let result = verify_and_report_update(&pb, package_name, &old_version).await;
                    return Ok(Some(result));
                }
                Ok(false) => {
                    // Unsupported or failed — fall through to cargo install subprocess
                    pb_status_dim(&pb, "Using", language.get_text("using_install_fallback"));
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
    // or primary for Git/Path/custom-features.
    let mut selector = CommandSelector::new(cargo_install_args, None);

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

        let output = run_cargo(&pb, selector.current()).await?;

        if output.status.success() {
            let result = verify_and_report_update(&pb, package_name, &old_version).await;
            // 命令成功但读不到新版本时，给当前路径一次重试机会（保留原行为）
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

        // binstall 第一次失败时立刻切到 cargo install 并就地重跑一次（这次
        // 回退不消耗 attempt 计数器）。关键：switch_to_fallback 之后
        // selector.current() 对后续每一次重试都返回 install——不再跑回
        // binstall（已修复"回退后重试又跑 binstall"的 bug）。
        if selector.switch_to_fallback() {
            pb_status_warn(&pb, "Fallback", language.get_text("binstall_failed_fallback"));
            let fb_output = run_cargo(&pb, selector.current()).await?;
            if fb_output.status.success() {
                return Ok(Some(verify_and_report_update(&pb, package_name, &old_version).await));
            }
            // 回退命令也失败:同样先排除"是用户取消造成的"。
            if cancel.load(Ordering::SeqCst) {
                pb_status_warn(&pb, "Aborted", &package_name.cyan().to_string());
                return Ok(None);
            }
            report_command_failure(&pb, package_name, &fb_output);
        } else {
            report_command_failure(&pb, package_name, &output);
        }

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
    use super::{build_args, CommandSelector};
    use crate::models::{InstallOpts, PackageSource};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[tokio::test]
    async fn cancelled_before_start_returns_none() {
        // 复现并锁定 bug:用户按下 Ctrl-C 后,update_package 必须立即返回
        // None 表示"被取消"——绝不当成更新失败,绝不 spawn cargo 子进程。
        use std::sync::{Arc, atomic::AtomicBool};
        let cancel = Arc::new(AtomicBool::new(true));
        let result = super::update_package(
            "cargo-fresh-no-such-package",
            Some("9.9.9"),
            &PackageSource::Crates,
            None,
            false, // dry_run
            false, // install_binstall
            false, // verbose
            cancel,
        )
        .await
        .expect("update_package 不应返回 Err");
        assert!(
            result.is_none(),
            "已取消时 update_package 必须返回 None,而不是一个结果"
        );
    }

    #[test]
    fn selector_starts_on_primary() {
        let primary = s(&["install", "--force", "mdbook"]);
        let sel = CommandSelector::new(primary.clone(), None);
        assert_eq!(sel.current(), primary.as_slice());
    }

    #[test]
    fn selector_without_fallback_always_primary() {
        // downloader 失败后只有 cargo install 作为唯一 subprocess 路径——
        // CommandSelector 不再有 binstall primary + install fallback 模式。
        let primary = s(&["install", "--force", "ripgrep"]);
        let mut sel = CommandSelector::new(primary.clone(), None);
        assert_eq!(sel.current(), primary.as_slice());
        assert!(!sel.switch_to_fallback());
        assert_eq!(sel.current(), primary.as_slice());
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
                "install", "--git", "https://github.com/x/y", "--rev", "abc",
                "--force", "y", "--features", "a"
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
        let src = PackageSource::Path { dir: "/tmp/p".into() };
        let got = build_args("p", None, &src, Some(&opts));
        assert_eq!(
            got,
            s(&[
                "install", "--path", "/tmp/p", "--force", "p",
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
