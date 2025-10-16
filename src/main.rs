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
use display::{print_results, print_update_selection, print_update_summary};
use locale::detect_language;
use models::{PackageInfo, UpdateResult};
use package::{
    check_package_updates, get_installed_packages, get_latest_version, is_stable_version,
};
use updater::{create_main_progress_bar, update_package};

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

    println!("{}", language.get_text("checking_packages").blue().bold());

    let mut packages = get_installed_packages().await?;

    if packages.is_empty() {
        println!("{}", language.get_text("no_packages_found").yellow());
        return Ok(());
    }

    println!(
        "{}",
        language
            .get_text("found_packages")
            .replace("{}", &packages.len().to_string())
    );

    // 首先检查稳定版本
    check_package_updates(&mut packages, cli.verbose, false).await?;

    // 检查预发布版本
    if !cli.include_prerelease {
        // 如果用户没有明确指定包含预发布版本，检查是否有预发布版本可用
        for package in packages.iter_mut() {
            if let Ok(Some(prerelease_version)) = get_latest_version(&package.name, true).await {
                if let Some(current_version) = &package.current_version {
                    if current_version != &prerelease_version {
                        // 检查是否是预发布版本
                        if !is_stable_version(&prerelease_version) {
                            package.latest_version = Some(prerelease_version);
                        }
                    }
                }
            }
        }
    }

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
        println!("{}", language.get_text("all_up_to_date").green().bold());
        return Ok(());
    }

    // 显示更新信息
    print_results(&packages, cli.updates_only, language);

    // 默认交互模式（除非用户指定 --no-interactive）
    if !cli.no_interactive {
        let selections = print_update_selection(&stable_updates, &prerelease_updates, language)?;

        if !selections.is_empty() {
            println!("\n{}", language.get_text("starting_update").blue().bold());

            let mut success_count = 0;
            let mut fail_count = 0;
            let mut update_results = Vec::new();
            let total_packages = selections.len();

            // 创建整体进度条
            let main_pb = create_main_progress_bar(total_packages);

            // 构建所有可更新的包列表
            let mut all_packages_to_update = stable_updates.clone();
            all_packages_to_update.extend(prerelease_updates.clone());

            for (i, &index) in selections.iter().enumerate() {
                let package_name = &all_packages_to_update[index].name;

                // 更新整体进度条消息
                main_pb.set_message(format!(
                    "{} ({}/{})",
                    language
                        .get_text("updating_package")
                        .replace("{}", package_name),
                    i + 1,
                    total_packages
                ));

                // 找到对应的包信息以获取目标版本
                let target_version = all_packages_to_update
                    .iter()
                    .find(|p| p.name == *package_name)
                    .and_then(|p| p.latest_version.as_ref())
                    .map(|v| v.as_str());

                match update_package(package_name, target_version).await {
                    Ok(result) => {
                        update_results.push(result.clone());
                        if result.success {
                            success_count += 1;
                        } else {
                            fail_count += 1;
                        }
                    }
                    Err(e) => {
                        main_pb.println(format!(
                            "❌ {} {}: {}",
                            package_name.red(),
                            language
                                .get_text("package_error")
                                .replace("{}", package_name),
                            e
                        ));
                        fail_count += 1;
                        update_results.push(UpdateResult::new(
                            package_name.clone(),
                            None,
                            None,
                            false,
                        ));
                    }
                }

                // 更新进度条
                main_pb.inc(1);
            }

            // 完成整体进度条
            main_pb.finish_with_message(language.get_text("update_completed"));

            // 显示更新摘要
            print_update_summary(&update_results, language);

            println!("\n{}", language.get_text("update_completed").green().bold());
            println!(
                "{}",
                language
                    .get_text("success_count")
                    .replace("{}", &success_count.to_string())
                    .green()
            );
            if fail_count > 0 {
                println!(
                    "{}",
                    language
                        .get_text("fail_count")
                        .replace("{}", &fail_count.to_string())
                        .red()
                );
            }
        } else {
            println!("{}", language.get_text("no_packages_selected").yellow());
        }
    } else {
        println!("\n{}", language.get_text("update_instructions").blue());
        println!("{}", language.get_text("interactive_instructions"));
    }

    Ok(())
}
