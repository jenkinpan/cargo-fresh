use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use colored::*;

// 模块声明
mod cli;
mod display;
mod locale;
mod models;
mod package;
mod updater;

// 导入模块
use cli::{Cli, Commands};
use display::{
    print_results, print_update_selection, print_update_summary, status, status_dim, status_err,
    status_warn,
};
use locale::detect_language;
use models::{PackageInfo, UpdateResult};
use package::{
    check_package_updates, exclude_packages, filter_packages, get_installed_packages,
    is_stable_version,
};
use updater::update_package;

#[tokio::main]
async fn main() -> Result<()> {
    // 检查是否作为 cargo 子命令运行
    let args: Vec<String> = std::env::args().collect();
    let cli = if args.get(1) == Some(&"fresh".to_string()) {
        // 移除 "fresh" 参数，保留其他参数
        Cli::parse_from(args.into_iter().skip(1))
    } else {
        Cli::parse()
    };

    // 检测系统语言
    let language = detect_language();

    // 全局取消标志：Ctrl-C 后 update 循环在"包之间"会停下，剩余包跳过。
    // 子进程内的取消（cargo install 已在跑）属于 P1-1（同步 IO 改 tokio::process）
    // 的范畴，这里只做"between packages"粒度。
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancel.store(true, Ordering::SeqCst);
            }
        });
    }

    // 处理子命令
    if let Some(command) = cli.command {
        match command {
            Commands::Completion { shell, cargo_fresh } => {
                if cargo_fresh {
                    Cli::generate_cargo_fresh_completion(shell);
                } else {
                    Cli::generate_completion(shell);
                }
                return Ok(());
            }
        }
    }

    status("Checking", language.get_text("checking_packages"));

    let mut packages = get_installed_packages().await?;

    if packages.is_empty() {
        status_warn("Note", language.get_text("no_packages_found"));
        return Ok(());
    }

    // 应用包过滤（先 filter 后 exclude）
    if let Some(filter_pattern) = &cli.filter {
        filter_packages(&mut packages, filter_pattern)?;
    }
    if !cli.exclude.is_empty() {
        exclude_packages(&mut packages, &cli.exclude)?;
    }
    if (cli.filter.is_some() || !cli.exclude.is_empty()) && packages.is_empty() {
        status_warn("Note", language.get_text("no_packages_found"));
        return Ok(());
    }

    status(
        "Found",
        &language
            .get_text("found_packages")
            .replace("{}", &packages.len().to_string()),
    );

    // 一次性拿稳定版 + 预发布版（sparse index 单次请求带回两者，
    // 失败时回退到 cargo search）。check_package_updates 内部会按优先级
    // 选 latest_version：优先 stable，无 stable 更新但有更新的预发布时填预发布
    check_package_updates(
        &mut packages,
        cli.verbose,
        cli.include_prerelease,
        cli.registry_url.clone(),
    )
    .await?;

    // 获取有稳定版本更新的包
    let stable_updates: Vec<&PackageInfo> = packages
        .iter()
        .filter(|p| {
            p.has_update()
                && p.latest_version
                    .as_ref()
                    .map(|v| is_stable_version(v))
                    .unwrap_or(false)
        })
        .collect();

    // 获取预发布版本更新的包
    let prerelease_updates: Vec<&PackageInfo> = packages
        .iter()
        .filter(|p| p.has_update() && p.is_prerelease())
        .collect();

    // 合并所有更新
    let mut all_updates = stable_updates.clone();
    all_updates.extend(prerelease_updates.clone());

    if all_updates.is_empty() {
        status("Finished", language.get_text("all_up_to_date"));
        return Ok(());
    }

    // 显示更新信息
    print_results(&packages, cli.updates_only, language);

    // 处理批量模式或交互模式
    let selections = if cli.batch {
        // 批量模式：自动选择所有有更新的包
        let mut all_indices = Vec::new();
        for (i, _) in all_updates.iter().enumerate() {
            all_indices.push(i);
        }
        all_indices
    } else if !cli.no_interactive {
        // 交互模式：让用户选择
        print_update_selection(&stable_updates, &prerelease_updates, language)?
    } else {
        // 非交互模式：不更新任何包
        Vec::new()
    };

    if !selections.is_empty() {
        println!();
        if cli.dry_run {
            status("Dry run", language.get_text("dry_run_summary"));
        } else {
            status("Updating", language.get_text("starting_update"));
        }

        // 记录更新开始时间
        let start_time = std::time::Instant::now();

        let mut success_count = 0;
        let mut fail_count = 0;
        let mut update_results = Vec::new();
        let total_packages = selections.len();

        // 构建所有可更新的包列表
        let mut all_packages_to_update = stable_updates.clone();
        all_packages_to_update.extend(prerelease_updates.clone());

        let mut aborted_at: Option<usize> = None;
        for (i, &index) in selections.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                aborted_at = Some(i);
                break;
            }
            let package_name = &all_packages_to_update[index].name;

            // 找到对应的包信息以获取目标版本和来源
            let selected_pkg = all_packages_to_update
                .iter()
                .find(|p| p.name == *package_name);
            let target_version = selected_pkg
                .and_then(|p| p.latest_version.as_ref())
                .map(|v| v.as_str());
            let source = selected_pkg
                .map(|p| p.source.clone())
                .unwrap_or(models::PackageSource::Crates);

            // 多包时给个 N/M 提示头，单包不冗余
            if total_packages > 1 {
                status_dim(
                    "Package",
                    &format!(
                        "{}/{} {}",
                        i + 1,
                        total_packages,
                        package_name.cyan()
                    ),
                );
            }

            match update_package(package_name, target_version, &source, cli.dry_run).await {
                Ok(result) => {
                    update_results.push(result.clone());
                    if result.success {
                        success_count += 1;
                    } else {
                        fail_count += 1;
                    }
                }
                Err(e) => {
                    status_err(
                        "Error",
                        &language.format_text(
                            "package_error",
                            &[
                                ("name", &package_name.red().to_string()),
                                ("error", &e.to_string()),
                            ],
                        ),
                    );
                    fail_count += 1;
                    update_results.push(UpdateResult::new(package_name.clone(), None, None, false));
                }
            }
        }

        // 计算总耗时
        let total_duration = start_time.elapsed();
        let duration_seconds = total_duration.as_secs();
        let duration_millis = total_duration.as_millis();

        // 显示更新摘要
        print_update_summary(&update_results, language);

        // 取消路径：打 Aborted 头，按已完成的数据打总结，并以退出码 130 退出
        if let Some(done) = aborted_at {
            status_warn(
                "Aborted",
                &language.format_text(
                    "aborted_by_user",
                    &[
                        ("done", &done.to_string()),
                        ("total", &total_packages.to_string()),
                    ],
                ),
            );
            std::process::exit(130);
        }

        // 单行 Finished 收尾，cargo 风格："X succeeded, Y failed in 3.4s"
        let success_text = language
            .get_text("success_count")
            .replace("{}", &success_count.to_string());
        let time_text = if duration_seconds > 0 {
            language
                .get_text("total_time_seconds")
                .replace("{}", &duration_seconds.to_string())
        } else {
            language
                .get_text("total_time_millis")
                .replace("{}", &duration_millis.to_string())
        };
        let summary = if fail_count > 0 {
            let fail_text = language
                .get_text("fail_count")
                .replace("{}", &fail_count.to_string());
            format!("{}, {}, {}", success_text, fail_text, time_text)
        } else {
            format!("{}, {}", success_text, time_text)
        };
        if fail_count > 0 {
            status_err("Finished", &summary);
        } else {
            status("Finished", &summary);
        }
    } else {
        status_dim("Note", language.get_text("no_packages_selected"));
    }

    Ok(())
}
