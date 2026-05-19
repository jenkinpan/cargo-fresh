use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::process::Output;
use tokio::process::Command;

use crate::display::{pb_status, pb_status_dim, pb_status_err, pb_status_warn, status, status_dim};
use crate::locale::detection::detect_language;
use crate::models::{
    InstallOpts, PackageSource, UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_TICK_MS,
    RETRY_DELAY_MS, VERSION_UPDATE_DELAY_MS,
};
use crate::package::{
    ensure_binstall_available, get_installed_version, invalidate_installed_version,
    is_binstall_available,
};

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
/// - `Crates`：`install`/`binstall --force <pkg> [--version V]`
/// - `Git`：`install --git URL [--rev REV] --force <pkg>`（binstall 不支持 git，强制 install）
/// - `Path`：`install --path DIR --force <pkg>`
/// - `opts`：如果提供且非默认，则在基础参数后追加 `--no-default-features` / `--all-features` / `--features a,b`
fn build_args(
    use_binstall: bool,
    package_name: &str,
    version: Option<&str>,
    source: &PackageSource,
    opts: Option<&InstallOpts>,
) -> Vec<String> {
    let mut args: Vec<String> = match source {
        PackageSource::Crates => {
            let subcmd = if use_binstall { "binstall" } else { "install" };
            match version {
                Some(v) => vec![
                    subcmd.into(),
                    "--force".into(),
                    package_name.into(),
                    "--version".into(),
                    v.into(),
                ],
                None => vec![subcmd.into(), "--force".into(), package_name.into()],
            }
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

async fn run_cargo(pb: &ProgressBar, args: &[String]) -> Result<Output> {
    pb_status_dim(pb, "Running", &format!("cargo {}", args.join(" ")));
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));
    let output = Command::new("cargo")
        .args(args)
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

pub async fn update_package(
    package_name: &str,
    target_version: Option<&str>,
    source: &PackageSource,
    dry_run: bool,
    install_binstall: bool,
) -> Result<UpdateResult> {
    let language = detect_language();
    let old_version = get_installed_version(package_name).await.ok().flatten();

    // 决定主命令的来源策略：
    // - Crates 源：先只读探测 binstall 可用性；不可用且 install_binstall=true
    //   时才主动安装 binstall（dry-run 下永远不动 toolchain）
    // - Crates 源 binstall 不可用、且用户未开启 install_binstall：打 Hint，
    //   走 `cargo install` 路径，不触发任何安装副作用
    // - Git / Path 源不走 binstall（binstall 仅支持 crates.io）
    let use_binstall = match source {
        PackageSource::Crates => {
            if is_binstall_available() {
                true
            } else if install_binstall && !dry_run {
                ensure_binstall_available().await.unwrap_or(false)
            } else {
                // 静默地提示一次，给 CI/审计场景留个线索而不打扰主输出
                status_dim(
                    "Hint",
                    language.get_text("binstall_hint"),
                );
                false
            }
        }
        _ => false,
    };

    // Task 6 将把这里的 None 换成真实 install_opts；本任务先用 None 保持可独立编译
    let primary_args = build_args(use_binstall, package_name, target_version, source, None);
    // 只有 Crates 源走 binstall 时才有 install 回退
    let fallback_args = if use_binstall {
        Some(build_args(false, package_name, target_version, source, None))
    } else {
        None
    };

    // dry-run：直接打印到 stdout（绕过 progress bar 避免 finish 时被清掉），
    // 立即返回成功结果，不调用 cargo。
    if dry_run {
        let marker = source.marker();
        let header = if marker.is_empty() {
            package_name.cyan().bold().to_string()
        } else {
            format!("{} {}", package_name.cyan().bold(), marker.dimmed())
        };
        status(
            "Would run",
            &format!("{}: cargo {}", header, primary_args.join(" ")),
        );
        if let Some(fb) = &fallback_args {
            status_dim(
                "Fallback",
                &format!("cargo {}", fb.join(" ")),
            );
        }
        return Ok(UpdateResult::new(
            package_name.to_string(),
            old_version.clone(),
            old_version,
            true,
        ));
    }

    let pb = create_progress_bar(package_name);
    let _pb_guard = PbGuard(&pb);
    let slow_handle = spawn_slow_warning(pb.clone(), package_name.to_string());
    // Drop slow_handle 时它仍可能在 sleep；中止它避免无意义提示在主流程结束后才打
    let _slow_guard = SlowGuard(slow_handle);
    if let Some(ref version) = old_version {
        pb_status_dim(
            &pb,
            language.get_text("current_version_label"),
            &version.blue().to_string(),
        );
    }

    match (source, use_binstall) {
        (PackageSource::Crates, true) => {
            pb_status_dim(&pb, "Using", language.get_text("using_binstall"));
        }
        (PackageSource::Crates, false) => {
            pb_status_dim(&pb, "Using", language.get_text("using_install_fallback"));
        }
        (PackageSource::Git { .. }, _) | (PackageSource::Path { .. }, _) => {
            pb_status_dim(
                &pb,
                "Using",
                &format!(
                    "{} {}",
                    language.get_text("using_install_fallback"),
                    source.marker().dimmed(),
            ));
        }
        (PackageSource::Unknown(_), _) => {
            // 不应该到这一步——main 不会把 unknown 包放进 selections。
            // 万一来到，给个明显的错误信息而不是默默走 cargo install。
            pb_status_warn(&pb, "Skip", &format!("{} {}", package_name, source.marker()));
            return Ok(UpdateResult::new(
                package_name.to_string(),
                old_version,
                None,
                false,
            ));
        }
    }

    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        if attempt > 1 {
            pb.set_message(language.format_text(
                "retry_attempt",
                &[
                    ("attempt", &attempt.to_string()),
                    ("name", &package_name.cyan().to_string()),
                ],
            ));
        }

        let output = run_cargo(&pb, &primary_args).await?;

        if output.status.success() {
            let result = verify_and_report_update(&pb, package_name, &old_version).await;
            // 命令成功但读不到新版本时，给主路径一次重试机会（保留原行为）
            if result.new_version.is_none() && attempt < MAX_RETRY_ATTEMPTS {
                pb_status_dim(&pb, "Retry", language.get_text("waiting_retry"));
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                continue;
            }
            return Ok(result);
        }

        // binstall 第一次失败：立刻尝试 install 回退（不消耗 attempt 计数器中的剩余次数）
        if let (Some(args), 1) = (fallback_args.as_ref(), attempt) {
            pb_status_warn(&pb, "Fallback", language.get_text("binstall_failed_fallback"));
            let fb_output = run_cargo(&pb, args).await?;
            if fb_output.status.success() {
                return Ok(verify_and_report_update(&pb, package_name, &old_version).await);
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

        return Ok(UpdateResult::new(
            package_name.to_string(),
            old_version,
            None,
            false,
        ));
    }

    Ok(UpdateResult::new(
        package_name.to_string(),
        old_version,
        None,
        false,
    ))
}

#[cfg(test)]
mod tests {
    use super::build_args;
    use crate::models::{InstallOpts, PackageSource};

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn crates_default_opts_no_extra_flags() {
        let got = build_args(false, "ripgrep", Some("14.1.1"), &PackageSource::Crates, None);
        assert_eq!(
            got,
            s(&["install", "--force", "ripgrep", "--version", "14.1.1"])
        );
    }

    #[test]
    fn crates_with_features() {
        let opts = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["pcre2".into(), "simd".into()],
        };
        let got = build_args(false, "ripgrep", None, &PackageSource::Crates, Some(&opts));
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
        let got = build_args(false, "x", None, &PackageSource::Crates, Some(&opts));
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
        let got = build_args(false, "y", None, &src, Some(&opts));
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
        let got = build_args(false, "p", None, &src, Some(&opts));
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
        let opts = InstallOpts::default();
        let got = build_args(true, "tool", None, &PackageSource::Crates, Some(&opts));
        assert_eq!(got, s(&["binstall", "--force", "tool"]));
    }
}
