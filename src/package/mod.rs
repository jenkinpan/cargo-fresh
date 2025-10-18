use anyhow::Result;
use colored::*;
use std::collections::HashSet;
use std::process::Command;
use std::sync::OnceLock;

use crate::locale::detection::detect_language;
use crate::models::{PackageInfo, PRERELEASE_KEYWORDS};

// 缓存 cargo binstall 的可用性状态
static BINSTALL_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// 检查 cargo binstall 是否可用（使用缓存）
pub fn is_binstall_available() -> bool {
    *BINSTALL_AVAILABLE.get_or_init(|| {
        Command::new("cargo")
            .args(["binstall", "--version"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
}

/// 安装 cargo binstall
pub async fn install_binstall() -> Result<bool> {
    let language = detect_language();
    println!("{}", language.get_text("installing_binstall").yellow());

    let output = Command::new("cargo")
        .args(["install", "cargo-binstall"])
        .output()?;

    if output.status.success() {
        println!(
            "✅ {}",
            language.get_text("binstall_installed_successfully").green()
        );
        Ok(true)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "❌ {}: {}",
            language.get_text("binstall_install_failed").red(),
            stderr
        );
        Ok(false)
    }
}

/// 确保 cargo binstall 可用，如果不可用则尝试安装
pub async fn ensure_binstall_available() -> Result<bool> {
    if is_binstall_available() {
        return Ok(true);
    }

    let language = detect_language();
    // 只在第一次检查时显示提示
    if BINSTALL_AVAILABLE.get().is_none() {
        println!("🔍 {}", language.get_text("binstall_not_found").yellow());
        println!(
            "⚡ {}",
            language.get_text("attempting_to_install_binstall").cyan()
        );
    }

    let result = install_binstall().await?;

    // 如果安装成功，更新缓存
    if result {
        let _ = BINSTALL_AVAILABLE.set(true);
    }

    Ok(result)
}

/// 根据模式过滤包
pub fn filter_packages(packages: &mut Vec<PackageInfo>, pattern: &str) -> Result<()> {
    if pattern.is_empty() {
        return Ok(());
    }

    // 简单的通配符匹配实现
    let pattern_lower = pattern.to_lowercase();
    packages.retain(|package| {
        let name_lower = package.name.to_lowercase();

        // 支持 * 通配符
        if pattern_lower.contains('*') {
            // 简单的模式匹配
            name_lower.contains(&pattern_lower.replace('*', ""))
                || name_lower.starts_with(&pattern_lower.replace('*', ""))
                || name_lower.ends_with(&pattern_lower.replace('*', ""))
        } else {
            name_lower.contains(&pattern_lower)
        }
    });

    Ok(())
}

pub async fn get_installed_packages() -> Result<Vec<PackageInfo>> {
    let output = Command::new("cargo").args(["install", "--list"]).output()?;

    if !output.status.success() {
        let language = detect_language();
        anyhow::bail!("{}", language.get_text("cargo_install_list_failed"));
    }

    let output_str = String::from_utf8(output.stdout)?;
    let mut packages = Vec::new();
    let mut seen_packages = HashSet::new();

    for line in output_str.lines() {
        if let Some((name, version)) = parse_package_line(line) {
            if !name.is_empty() && !version.is_empty() && seen_packages.insert(name) {
                packages.push(PackageInfo::new(
                    name.to_string(),
                    Some(version.to_string()),
                ));
            }
        }
    }

    Ok(packages)
}

pub fn parse_package_line(line: &str) -> Option<(&str, &str)> {
    if !line.contains(" v") || !line.contains(":") {
        return None;
    }

    let parts: Vec<&str> = line.split(" v").collect();
    if parts.len() != 2 {
        return None;
    }

    let package_name = parts[0].trim();
    let version_part = parts[1].split(':').next()?.trim();

    if package_name.is_empty() || version_part.is_empty() {
        return None;
    }

    Some((package_name, version_part))
}

pub async fn get_installed_version(package_name: &str) -> Result<Option<String>> {
    let output = Command::new("cargo").args(["install", "--list"]).output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let output_str = String::from_utf8(output.stdout)?;

    for line in output_str.lines() {
        if line.contains(package_name) {
            if let Some((name, version)) = parse_package_line(line) {
                if name == package_name {
                    return Ok(Some(version.to_string()));
                }
            }
        }
    }

    Ok(None)
}

pub fn extract_version_from_line(line: &str) -> Option<String> {
    line.find("= \"").and_then(|start| {
        line[start + 3..]
            .find("\"")
            .map(|end| line[start + 3..start + 3 + end].to_string())
    })
}

pub fn is_stable_version(version: &str) -> bool {
    !PRERELEASE_KEYWORDS
        .iter()
        .any(|&keyword| version.contains(keyword))
}

pub async fn get_latest_version(
    package_name: &str,
    include_prerelease: bool,
) -> Result<Option<String>> {
    let output = Command::new("cargo")
        .args(["search", package_name, "--limit", "10"])
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let output_str = String::from_utf8(output.stdout)?;
    let package_prefix = format!("{} =", package_name);

    // 查找精确匹配的包名
    for line in output_str.lines() {
        if line.starts_with(&package_prefix) && line.contains("\"") {
            if let Some(version) = extract_version_from_line(line) {
                if include_prerelease || is_stable_version(&version) {
                    return Ok(Some(version));
                }
            }
        }
    }

    // 如果没有找到稳定版本且不包含预发布版本，返回None
    if !include_prerelease {
        return Ok(None);
    }

    // 如果包含预发布版本但没有找到精确匹配，返回第一个匹配的版本
    for line in output_str.lines() {
        if line.starts_with(&package_prefix) && line.contains("\"") {
            if let Some(version) = extract_version_from_line(line) {
                return Ok(Some(version));
            }
        }
    }

    Ok(None)
}

pub async fn check_package_updates(
    packages: &mut [PackageInfo],
    verbose: bool,
    include_prerelease: bool,
) -> Result<()> {
    let language = detect_language();

    // 创建并发任务来检查所有包
    let mut handles = Vec::new();

    for (index, package) in packages.iter().enumerate() {
        let package_name = package.name.clone();
        let handle = tokio::spawn(async move {
            if verbose {
                println!(
                    "{} {}...",
                    language.get_text("checking_package"),
                    package_name.cyan()
                );
            }

            let result = get_latest_version(&package_name, include_prerelease).await;
            (index, package_name, result)
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    for handle in handles {
        match handle.await {
            Ok((index, package_name, result)) => match result {
                Ok(Some(version)) => {
                    packages[index].latest_version = Some(version.clone());
                    if verbose {
                        println!(
                            "  {} {}: {}",
                            package_name,
                            language.get_text("latest_version"),
                            version.green()
                        );
                    }
                }
                Ok(None) => {
                    if verbose {
                        println!(
                            "  {} {}",
                            package_name.red(),
                            language.get_text("unable_to_get_latest_version")
                        );
                    }
                }
                Err(e) => {
                    if verbose {
                        println!(
                            "  {} {}: {}",
                            package_name.red(),
                            language.get_text("check_failed"),
                            e
                        );
                    }
                }
            },
            Err(e) => {
                if verbose {
                    println!("Task failed: {}", e);
                }
            }
        }
    }

    Ok(())
}
