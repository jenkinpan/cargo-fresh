use anyhow::Result;
use clap::Parser;
use colored::*;

// 模块声明
mod cli;
mod display;
mod models;
mod package;
mod updater;

// 导入模块
use cli::Cli;
use display::{print_results, print_update_selection, print_update_summary};
use models::{PackageInfo, UpdateResult};
use package::{
    check_package_updates, get_installed_packages, get_latest_version, is_stable_version,
};
use updater::{create_main_progress_bar, update_package};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 处理 shell 补全生成
    if let Some(shell) = cli.completion {
        Cli::generate_completion(shell);
        return Ok(());
    }

    println!("{}", "检查全局安装的 Cargo 包更新...".blue().bold());

    let mut packages = get_installed_packages().await?;

    if packages.is_empty() {
        println!("{}", "没有找到已安装的包".yellow());
        return Ok(());
    }

    println!("找到 {} 个已安装的包", packages.len());

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
        println!("{}", "所有包都已是最新版本！".green().bold());
        return Ok(());
    }

    // 显示更新信息
    print_results(&packages, cli.updates_only);

    // 默认交互模式（除非用户指定 --no-interactive）
    if !cli.no_interactive {
        let selections = print_update_selection(&stable_updates, &prerelease_updates)?;

        if !selections.is_empty() {
            println!("\n{}", "开始更新选中的包...".blue().bold());

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
                    "正在更新 {} ({}/{})",
                    package_name,
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
                            main_pb.println(format!("✅ {} 更新成功", package_name.green()));
                        } else {
                            fail_count += 1;
                            main_pb.println(format!("❌ {} 更新失败", package_name.red()));
                        }
                    }
                    Err(e) => {
                        main_pb.println(format!("❌ {} 更新出错: {}", package_name.red(), e));
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
            main_pb.finish_with_message("所有包更新完成！");

            // 显示更新摘要
            print_update_summary(&update_results);

            println!("\n{}", "更新完成！".green().bold());
            println!("成功: {} 个包", success_count.to_string().green());
            if fail_count > 0 {
                println!("失败: {} 个包", fail_count.to_string().red());
            }
        } else {
            println!("{}", "未选择任何包进行更新".yellow());
        }
    } else {
        println!(
            "\n{}",
            "要更新包，请使用: cargo install --force <package_name>".blue()
        );
        println!("或者移除 --no-interactive 参数进行交互式更新");
    }

    Ok(())
}
