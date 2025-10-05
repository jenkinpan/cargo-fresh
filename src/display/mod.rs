use colored::*;
use dialoguer::{Confirm, MultiSelect};

use crate::models::{PackageInfo, UpdateResult};

pub fn format_version_info(old: &Option<String>, new: &Option<String>) -> String {
    match (old, new) {
        (Some(old), Some(new)) if old != new => {
            format!("{} → {}", old.red(), new.green())
        }
        (Some(old), Some(_)) => {
            format!("{} (版本未改变)", old.yellow())
        }
        (Some(old), None) => {
            format!("{} → 未知版本", old.red())
        }
        (None, Some(new)) => {
            format!("未知版本 → {}", new.green())
        }
        _ => "版本信息未知".to_string(),
    }
}

pub fn print_results(packages: &[PackageInfo], updates_only: bool) {
    let mut has_updates = false;

    for package in packages {
        if updates_only && !package.has_update() {
            continue;
        }

        if package.has_update() {
            has_updates = true;
            println!("{} 有更新可用", package.name.yellow().bold());
            if let Some(current) = &package.current_version {
                println!("  当前版本: {}", current.red());
            }
            if let Some(latest) = &package.latest_version {
                println!("  最新版本: {}", latest.green());
            }
        } else if !updates_only {
            println!("{} 已是最新版本", package.name.green());
            if let Some(current) = &package.current_version {
                println!("  版本: {}", current.green());
            }
        }
    }

    if updates_only && !has_updates {
        println!("{}", "所有包都已是最新版本！".green().bold());
    }
}

pub fn print_update_summary(update_results: &[UpdateResult]) {
    if update_results.is_empty() {
        return;
    }

    println!("\n{}", "📋 更新摘要".blue().bold());
    println!("{}", "=".repeat(50).blue());

    let mut success_updates = Vec::new();
    let mut failed_updates = Vec::new();

    for result in update_results {
        if result.success {
            success_updates.push(result);
        } else {
            failed_updates.push(result);
        }
    }

    // 显示成功的更新
    if !success_updates.is_empty() {
        println!("\n{}", "✅ 成功更新的包:".green().bold());
        for result in &success_updates {
            println!(
                "  • {}: {}",
                result.package_name.cyan(),
                format_version_info(&result.old_version, &result.new_version)
            );
        }
    }

    // 显示失败的更新
    if !failed_updates.is_empty() {
        println!("\n{}", "❌ 更新失败的包:".red().bold());
        for result in &failed_updates {
            if let Some(old) = &result.old_version {
                println!(
                    "  • {}: {} (更新失败)",
                    result.package_name.cyan(),
                    old.red()
                );
            } else {
                println!("  • {}: 更新失败", result.package_name.cyan());
            }
        }
    }

    println!("{}", "=".repeat(50).blue());
}

pub fn print_update_selection(
    stable_updates: &[&PackageInfo],
    prerelease_updates: &[&PackageInfo],
) -> Result<Vec<usize>, anyhow::Error> {
    println!("\n{}", "检测到以下包有更新:".yellow().bold());

    // 显示稳定版本更新
    if !stable_updates.is_empty() {
        println!("{}", "稳定版本更新:".green().bold());
        for package in stable_updates {
            println!(
                "  • {} ({} → {})",
                package.name.cyan(),
                package
                    .current_version
                    .as_ref()
                    .unwrap_or(&"未知".to_string())
                    .red(),
                package
                    .latest_version
                    .as_ref()
                    .unwrap_or(&"未知".to_string())
                    .green()
            );
        }
    }

    // 显示预发布版本更新
    if !prerelease_updates.is_empty() {
        println!("{}", "预发布版本更新:".yellow().bold());
        for package in prerelease_updates {
            println!(
                "  • {} ({} → {}) {}",
                package.name.cyan(),
                package
                    .current_version
                    .as_ref()
                    .unwrap_or(&"未知".to_string())
                    .red(),
                package
                    .latest_version
                    .as_ref()
                    .unwrap_or(&"未知".to_string())
                    .yellow(),
                "⚠️ 预发布版本".yellow()
            );
        }
    }

    // 询问是否要更新
    if !Confirm::new()
        .with_prompt("是否要更新这些包？")
        .default(true)
        .interact()?
    {
        return Ok(vec![]);
    }

    // 如果有预发布版本，询问是否包含
    let mut packages_to_update = stable_updates.to_vec();
    if !prerelease_updates.is_empty()
        && Confirm::new()
            .with_prompt("是否包含预发布版本更新？")
            .default(false)
            .interact()?
    {
        packages_to_update.extend(prerelease_updates);
    }

    // 让用户选择要更新的包
    let package_names: Vec<String> = packages_to_update.iter().map(|p| p.name.clone()).collect();

    let selections = MultiSelect::new()
        .with_prompt("选择要更新的包（使用空格选择，回车确认）")
        .items(&package_names)
        .interact()?;

    Ok(selections)
}
