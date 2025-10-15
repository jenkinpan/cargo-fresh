use anyhow::Result;
use colored::*;
use std::collections::HashSet;
use std::process::Command;

use crate::locale::detection::detect_language;
use crate::models::{PackageInfo, PRERELEASE_KEYWORDS};

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

    for package in packages.iter_mut() {
        if verbose {
            println!(
                "{} {}...",
                language.get_text("checking_package"),
                package.name.cyan()
            );
        }

        match get_latest_version(&package.name, include_prerelease).await {
            Ok(Some(version)) => {
                package.latest_version = Some(version);
                if verbose {
                    println!(
                        "  {} {}: {}",
                        package.name,
                        language.get_text("latest_version"),
                        package.latest_version.as_ref().unwrap().green()
                    );
                }
            }
            Ok(None) => {
                if verbose {
                    println!(
                        "  {} {}",
                        package.name.red(),
                        language.get_text("unable_to_get_latest_version")
                    );
                }
            }
            Err(e) => {
                if verbose {
                    println!(
                        "  {} {}: {}",
                        package.name.red(),
                        language.get_text("check_failed"),
                        e
                    );
                }
            }
        }
    }
    Ok(())
}
