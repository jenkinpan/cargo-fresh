use colored::*;
use dialoguer::{Confirm, MultiSelect};

use crate::locale::Language;
use crate::models::{PackageInfo, UpdateResult};

/// 格式化包版本信息
fn format_package_version(package: &PackageInfo, language: Language) -> String {
    let current = package
        .current_version
        .as_deref()
        .unwrap_or(language.get_text("unknown"));
    let latest = package
        .latest_version
        .as_deref()
        .unwrap_or(language.get_text("unknown"));

    format!(
        "{} ({} → {})",
        package.name.cyan(),
        current.red(),
        latest.green()
    )
}

pub fn format_version_info(
    old: &Option<String>,
    new: &Option<String>,
    language: Language,
) -> String {
    match (old, new) {
        (Some(old), Some(new)) if old != new => {
            format!("{} → {}", old.red(), new.green())
        }
        (Some(old), Some(_)) => {
            format!(
                "{} ({})",
                old.yellow(),
                language.get_text("version_unchanged")
            )
        }
        (Some(old), None) => {
            format!("{} → {}", old.red(), language.get_text("unknown_version"))
        }
        (None, Some(new)) => {
            format!("{} → {}", language.get_text("unknown_version"), new.green())
        }
        _ => language.get_text("version_info_unknown").to_string(),
    }
}

pub fn print_results(packages: &[PackageInfo], updates_only: bool, language: Language) {
    let mut has_updates = false;

    for package in packages {
        if updates_only && !package.has_update() {
            continue;
        }

        if package.has_update() {
            has_updates = true;
            println!(
                "{}",
                language
                    .get_text("package_has_update")
                    .replace("{}", &package.name)
                    .yellow()
                    .bold()
            );
            if let Some(current) = &package.current_version {
                println!(
                    "  {} {}",
                    language.get_text("current_version"),
                    current.red()
                );
            }
            if let Some(latest) = &package.latest_version {
                println!(
                    "  {} {}",
                    language.get_text("latest_version"),
                    latest.green()
                );
            }
        } else if !updates_only {
            println!(
                "{}",
                language
                    .get_text("package_up_to_date")
                    .replace("{}", &package.name)
                    .green()
            );
            if let Some(current) = &package.current_version {
                println!("  {} {}", language.get_text("version"), current.green());
            }
        }
    }

    if updates_only && !has_updates {
        println!("{}", language.get_text("all_up_to_date").green().bold());
    }
}

pub fn print_update_summary(update_results: &[UpdateResult], language: Language) {
    if update_results.is_empty() {
        return;
    }

    println!("\n{}", language.get_text("update_summary").blue().bold());
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
        println!(
            "\n{}",
            language.get_text("successful_updates").green().bold()
        );
        for result in &success_updates {
            println!(
                "  • {}: {}",
                result.package_name.cyan(),
                format_version_info(&result.old_version, &result.new_version, language)
            );
        }
    }

    // 显示失败的更新
    if !failed_updates.is_empty() {
        println!("\n{}", language.get_text("failed_updates").red().bold());
        for result in &failed_updates {
            if let Some(old) = &result.old_version {
                println!(
                    "  • {}: {} ({})",
                    result.package_name.cyan(),
                    old.red(),
                    language.get_text("update_failed")
                );
            } else {
                println!(
                    "  • {}: {}",
                    result.package_name.cyan(),
                    language.get_text("update_failed")
                );
            }
        }
    }

    println!("{}", "=".repeat(50).blue());
}

pub fn print_update_selection(
    stable_updates: &[&PackageInfo],
    prerelease_updates: &[&PackageInfo],
    language: Language,
) -> Result<Vec<usize>, anyhow::Error> {
    println!(
        "\n{}",
        language.get_text("updates_detected").yellow().bold()
    );

    // 显示稳定版本更新
    if !stable_updates.is_empty() {
        println!("{}", language.get_text("stable_updates").green().bold());
        for package in stable_updates {
            println!("  • {}", format_package_version(package, language));
        }
    }

    // 显示预发布版本更新
    if !prerelease_updates.is_empty() {
        println!(
            "{}",
            language.get_text("prerelease_updates").yellow().bold()
        );
        for package in prerelease_updates {
            println!(
                "  • {} ({} → {}) {}",
                package.name.cyan(),
                package
                    .current_version
                    .as_ref()
                    .unwrap_or(&language.get_text("unknown").to_string())
                    .red(),
                package
                    .latest_version
                    .as_ref()
                    .unwrap_or(&language.get_text("unknown").to_string())
                    .yellow(),
                language.get_text("prerelease_warning").yellow()
            );
        }
    }

    // 询问是否要更新
    if !Confirm::new()
        .with_prompt(language.get_text("update_question"))
        .default(true)
        .interact()?
    {
        return Ok(vec![]);
    }

    // 如果有预发布版本，询问是否包含
    let mut packages_to_update = stable_updates.to_vec();
    if !prerelease_updates.is_empty()
        && Confirm::new()
            .with_prompt(language.get_text("include_prerelease_question"))
            .default(false)
            .interact()?
    {
        packages_to_update.extend(prerelease_updates);
    }

    // 让用户选择要更新的包
    let package_names: Vec<String> = packages_to_update.iter().map(|p| p.name.clone()).collect();

    let selections = MultiSelect::new()
        .with_prompt(language.get_text("select_packages"))
        .items(&package_names)
        .interact()?;

    Ok(selections)
}
