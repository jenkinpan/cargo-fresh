use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command;

use crate::locale::detection::detect_language;
use crate::models::{
    UpdateResult, MAX_RETRY_ATTEMPTS, PROGRESS_BAR_WIDTH, PROGRESS_TICK_MS, RETRY_DELAY_MS,
    VERSION_UPDATE_DELAY_MS,
};
use crate::package::get_installed_version;

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
                "{{bar:{}.green/blue}} {{pos}}/{{len}} {{msg}}",
                PROGRESS_BAR_WIDTH
            ))
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
    );
    pb.set_message(
        language
            .get_text("updating_package_progress")
            .replace("{}", "packages"),
    );
    pb
}

pub async fn update_package(
    package_name: &str,
    target_version: Option<&str>,
) -> Result<UpdateResult> {
    let language = detect_language();
    let pb = create_progress_bar(package_name);

    // 获取更新前的版本
    let old_version = get_installed_version(package_name).await.ok().flatten();
    if let Some(ref version) = old_version {
        pb.println(format!(
            "{} {}",
            language.get_text("current_version_label"),
            version.blue()
        ));
    }

    // 构建安装命令
    let mut args = vec!["install", "--force"];
    if let Some(version) = target_version {
        args.push(package_name);
        args.extend(&["--version", version]);
        pb.println(format!(
            "{} cargo install --force {} --version {}",
            language.get_text("executing_command"),
            package_name,
            version
        ));
    } else {
        args.push(package_name);
        pb.println(format!(
            "{} cargo install --force {}",
            language.get_text("executing_command"),
            package_name
        ));
    }

    // 尝试更新，最多重试3次
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        if attempt > 1 {
            pb.set_message(format!(
                "{} {} {}",
                language.get_text("retry_attempt").replace("{}", "").trim(),
                attempt,
                package_name.cyan()
            ));
        }

        // 启动进度条
        pb.enable_steady_tick(std::time::Duration::from_millis(PROGRESS_TICK_MS));

        let output = Command::new("cargo")
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()?;

        // 停止进度条
        pb.finish_and_clear();

        // 检查是否有真正的错误（非编译输出）
        let stderr = String::from_utf8_lossy(&output.stderr);

        // 只有在命令失败时才显示错误信息
        if !output.status.success() && !stderr.is_empty() {
            pb.println(format!("{} {}", language.get_text("error_message"), stderr));
        }

        if output.status.success() {
            pb.println(format!(
                "{}",
                language
                    .get_text("package_update_success")
                    .replace("{}", &package_name.green().to_string())
            ));

            // 等待系统更新
            tokio::time::sleep(tokio::time::Duration::from_millis(VERSION_UPDATE_DELAY_MS)).await;

            // 验证更新是否真的成功
            if let Ok(Some(new_version)) = get_installed_version(package_name).await {
                if old_version.as_ref() != Some(&new_version) {
                    pb.println(format!(
                        "{}",
                        language
                            .get_text("package_updated_version")
                            .replace("{}", &package_name.green().to_string())
                            .replace(
                                "{}",
                                &old_version
                                    .as_ref()
                                    .unwrap_or(&language.get_text("unknown_version").to_string())
                                    .red()
                                    .to_string()
                            )
                            .replace("{}", &new_version.green().to_string())
                    ));
                    return Ok(UpdateResult::new(
                        package_name.to_string(),
                        old_version.clone(),
                        Some(new_version),
                        true,
                    ));
                } else {
                    pb.println(format!(
                        "{}",
                        language
                            .get_text("package_version_unchanged")
                            .replace("{}", &package_name.yellow().to_string())
                    ));
                    return Ok(UpdateResult::new(
                        package_name.to_string(),
                        old_version.clone(),
                        old_version,
                        true,
                    ));
                }
            } else {
                pb.println(format!(
                    "{}",
                    language
                        .get_text("package_update_verification_failed")
                        .replace("{}", &package_name.yellow().to_string())
                ));
                if attempt < MAX_RETRY_ATTEMPTS {
                    pb.println(language.get_text("waiting_retry"));
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                return Ok(UpdateResult::new(
                    package_name.to_string(),
                    old_version.clone(),
                    None,
                    true,
                ));
            }
        } else {
            pb.println(format!(
                "{}",
                language
                    .get_text("package_update_failed")
                    .replace("{}", &package_name.red().to_string())
                    .replace("{}", &output.status.code().unwrap_or(-1).to_string())
            ));
            if !stderr.is_empty() {
                pb.println(format!(
                    "{} {}",
                    language.get_text("error_details"),
                    stderr.red()
                ));
            }

            if attempt < MAX_RETRY_ATTEMPTS {
                pb.println(language.get_text("waiting_retry"));
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                continue;
            }
            return Ok(UpdateResult::new(
                package_name.to_string(),
                old_version.clone(),
                None,
                false,
            ));
        }
    }

    Ok(UpdateResult::new(
        package_name.to_string(),
        old_version.clone(),
        None,
        false,
    ))
}
