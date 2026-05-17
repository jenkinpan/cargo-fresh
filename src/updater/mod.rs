use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::process::{Command, Output};

use crate::display::{pb_status, pb_status_dim, pb_status_err, pb_status_warn, status, status_dim};
use crate::locale::detection::detect_language;
use crate::models::{
    PackageSource, UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_BAR_WIDTH, PROGRESS_TICK_MS,
    RETRY_DELAY_MS, VERSION_UPDATE_DELAY_MS,
};
use crate::package::{
    ensure_binstall_available, get_installed_version, invalidate_installed_version,
    is_binstall_available,
};

pub fn create_progress_bar(package_name: &str) -> ProgressBar {
    let language = detect_language();
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(format!(
        "{} {}",
        language
            .get_text("updating_package_progress")
            .replace("{}", "")
            .trim(),
        package_name.cyan()
    ));
    pb
}

pub fn create_main_progress_bar(total: usize) -> ProgressBar {
    let language = detect_language();
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(&format!(
                "{{spinner:.green}} {{bar:{}.green/blue}} {{pos}}/{{len}} {{msg}}",
                PROGRESS_BAR_WIDTH
            ))
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  ")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(
        language
            .get_text("updating_package_progress")
            .replace("{}", "packages"),
    );
    pb
}

/// 根据来源类型构造 cargo 子命令参数。
///
/// - `Crates`：`install`/`binstall --force <pkg> [--version V]`
/// - `Git`：`install --git URL [--rev REV] --force <pkg>`（binstall 不支持 git，强制 install）
/// - `Path`：`install --path DIR --force <pkg>`
fn build_args<'a>(
    use_binstall: bool,
    package_name: &'a str,
    version: Option<&'a str>,
    source: &'a PackageSource,
) -> Vec<&'a str> {
    match source {
        PackageSource::Crates => {
            let subcmd = if use_binstall { "binstall" } else { "install" };
            match version {
                Some(v) => vec![subcmd, "--force", package_name, "--version", v],
                None => vec![subcmd, "--force", package_name],
            }
        }
        PackageSource::Git { url, rev } => {
            let mut args = vec!["install", "--git", url.as_str()];
            if let Some(r) = rev {
                args.push("--rev");
                args.push(r.as_str());
            }
            args.push("--force");
            args.push(package_name);
            args
        }
        PackageSource::Path { dir } => {
            vec!["install", "--path", dir.as_str(), "--force", package_name]
        }
    }
}

fn run_cargo(pb: &ProgressBar, args: &[&str]) -> Result<Output> {
    pb_status_dim(pb, "Running", &format!("cargo {}", args.join(" ")));
    pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));
    let output = Command::new("cargo")
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;
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
) -> Result<UpdateResult> {
    let language = detect_language();
    let old_version = get_installed_version(package_name).await.ok().flatten();

    // 决定主命令的来源策略：
    // - Crates 源在非 dry-run 下确保 binstall 可用（必要时安装它）
    // - Crates 源在 dry-run 下用只读探测，避免副作用
    // - Git / Path 源不走 binstall（binstall 仅支持 crates.io）
    let use_binstall = match source {
        PackageSource::Crates => {
            if dry_run {
                is_binstall_available()
            } else {
                ensure_binstall_available().await.unwrap_or(false)
            }
        }
        _ => false,
    };

    let primary_args = build_args(use_binstall, package_name, target_version, source);
    // 只有 Crates 源走 binstall 时才有 install 回退
    let fallback_args = if use_binstall {
        Some(build_args(false, package_name, target_version, source))
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

        let output = run_cargo(&pb, &primary_args)?;

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
            let fb_output = run_cargo(&pb, args)?;
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
