use anyhow::Result;
use clap::{CommandFactory, Parser};
use colored::*;
use dialoguer::{Confirm, MultiSelect};
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command;

// 常量定义
const PRERELEASE_KEYWORDS: &[&str] = &["alpha", "beta", "rc"];
const MAX_RETRY_ATTEMPTS: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const VERSION_UPDATE_DELAY_MS: u64 = 1000;

#[derive(Parser)]
#[command(name = "pkg-checker")]
#[command(about = "检查全局安装的Cargo包更新")]
#[command(version)]
struct Cli {
    /// 显示详细信息
    #[arg(short, long)]
    verbose: bool,

    /// 只显示有更新的包
    #[arg(short, long)]
    updates_only: bool,

    /// 非交互模式（默认是交互模式）
    #[arg(long)]
    no_interactive: bool,

    /// 包含预发布版本（alpha、beta、rc等）
    #[arg(long)]
    include_prerelease: bool,

    /// 生成 shell 补全脚本
    #[arg(long, value_name = "SHELL")]
    completion: Option<String>,
}

#[derive(Debug)]
struct PackageInfo {
    name: String,
    current_version: Option<String>,
    latest_version: Option<String>,
}

impl PackageInfo {
    fn has_update(&self) -> bool {
        matches!(
            (&self.current_version, &self.latest_version),
            (Some(current), Some(latest)) if current != latest
        )
    }

    fn is_prerelease(&self) -> bool {
        self.latest_version
            .as_ref()
            .map(|v| {
                PRERELEASE_KEYWORDS
                    .iter()
                    .any(|&keyword| v.contains(keyword))
            })
            .unwrap_or(false)
    }
}

async fn get_installed_packages() -> Result<Vec<PackageInfo>> {
    let output = Command::new("cargo").args(["install", "--list"]).output()?;

    if !output.status.success() {
        anyhow::bail!("执行 cargo install --list 失败");
    }

    let output_str = String::from_utf8(output.stdout)?;
    let mut packages = Vec::new();
    let mut seen_packages = std::collections::HashSet::new();

    for line in output_str.lines() {
        // 解析格式: "package_name v0.1.0:"
        if line.contains(" v") && line.contains(":") {
            let parts: Vec<&str> = line.split(" v").collect();
            if parts.len() == 2 {
                let package_name = parts[0].trim();
                let version_part = parts[1].split(':').next().unwrap_or("");
                let version = version_part.trim();

                if !package_name.is_empty()
                    && !version.is_empty()
                    && seen_packages.insert(package_name)
                {
                    packages.push(PackageInfo {
                        name: package_name.to_string(),
                        current_version: Some(version.to_string()),
                        latest_version: None,
                    });
                }
            }
        }
    }

    Ok(packages)
}

fn extract_version_from_line(line: &str) -> Option<String> {
    line.find("= \"").and_then(|start| {
        line[start + 3..]
            .find("\"")
            .map(|end| line[start + 3..start + 3 + end].to_string())
    })
}

fn is_stable_version(version: &str) -> bool {
    !PRERELEASE_KEYWORDS
        .iter()
        .any(|&keyword| version.contains(keyword))
}

async fn get_latest_version(
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

async fn update_package(package_name: &str, target_version: Option<&str>) -> Result<bool> {
    // 创建进度条
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(format!("正在更新 {}...", package_name.cyan()));

    // 获取更新前的版本
    let old_version = get_installed_version(package_name).await.ok().flatten();
    if let Some(ref version) = old_version {
        pb.println(format!("当前版本: {}", version.blue()));
    }

    // 构建安装命令
    let mut args = vec!["install", "--force"];
    if let Some(version) = target_version {
        args.push(package_name);
        args.extend(&["--version", version]);
        pb.println(format!(
            "执行命令: cargo install --force {} --version {}",
            package_name, version
        ));
    } else {
        args.push(package_name);
        pb.println(format!("执行命令: cargo install --force {}", package_name));
    }

    // 尝试更新，最多重试3次
    for attempt in 1..=MAX_RETRY_ATTEMPTS {
        if attempt > 1 {
            pb.set_message(format!(
                "重试第 {} 次更新 {}...",
                attempt,
                package_name.cyan()
            ));
        }

        // 启动进度条
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

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
            pb.println(format!("错误信息: {}", stderr));
        }

        if output.status.success() {
            pb.println(format!("✅ {} 更新命令执行成功", package_name.green()));

            // 等待系统更新
            tokio::time::sleep(tokio::time::Duration::from_millis(VERSION_UPDATE_DELAY_MS)).await;

            // 验证更新是否真的成功
            if let Ok(Some(new_version)) = get_installed_version(package_name).await {
                if old_version.as_ref() != Some(&new_version) {
                    pb.println(format!(
                        "✅ {} 已更新: {} → {}",
                        package_name.green(),
                        old_version.unwrap_or_else(|| "未知".to_string()).red(),
                        new_version.green()
                    ));
                    return Ok(true);
                } else {
                    pb.println(format!(
                        "⚠️ {} 版本未改变，可能已经是最新版本",
                        package_name.yellow()
                    ));
                    return Ok(true);
                }
            } else {
                pb.println(format!(
                    "⚠️ {} 更新命令成功但无法验证新版本",
                    package_name.yellow()
                ));
                if attempt < MAX_RETRY_ATTEMPTS {
                    pb.println("等待后重试...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                    continue;
                }
                return Ok(true); // 仍然认为成功，因为命令执行成功了
            }
        } else {
            pb.println(format!(
                "❌ {} 更新失败 (退出码: {})",
                package_name.red(),
                output.status.code().unwrap_or(-1)
            ));
            if !stderr.is_empty() {
                pb.println(format!("错误详情: {}", stderr.red()));
            }

            if attempt < MAX_RETRY_ATTEMPTS {
                pb.println("等待后重试...");
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
                continue;
            }
            return Ok(false);
        }
    }

    Ok(false)
}

async fn get_installed_version(package_name: &str) -> Result<Option<String>> {
    let output = Command::new("cargo").args(["install", "--list"]).output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let output_str = String::from_utf8(output.stdout)?;

    for line in output_str.lines() {
        if line.contains(package_name) && line.contains(" v") && line.contains(":") {
            let parts: Vec<&str> = line.split(" v").collect();
            if parts.len() == 2 {
                let version_part = parts[1].split(':').next().unwrap_or("");
                let version = version_part.trim();
                if !version.is_empty() {
                    return Ok(Some(version.to_string()));
                }
            }
        }
    }

    Ok(None)
}

async fn check_package_updates(
    packages: &mut [PackageInfo],
    verbose: bool,
    include_prerelease: bool,
) -> Result<()> {
    for package in packages.iter_mut() {
        if verbose {
            println!("检查 {}...", package.name.cyan());
        }

        match get_latest_version(&package.name, include_prerelease).await {
            Ok(Some(version)) => {
                package.latest_version = Some(version);
                if verbose {
                    println!(
                        "  {} 最新版本: {}",
                        package.name,
                        package.latest_version.as_ref().unwrap().green()
                    );
                }
            }
            Ok(None) => {
                if verbose {
                    println!("  {} 无法获取最新版本信息", package.name.red());
                }
            }
            Err(e) => {
                if verbose {
                    println!("  {} 检查失败: {}", package.name.red(), e);
                }
            }
        }
    }
    Ok(())
}

fn print_results(packages: &[PackageInfo], updates_only: bool) {
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

fn generate_completion(shell: String) {
    let mut cmd = Cli::command();
    let shell = shell.to_lowercase();

    let shell_type = match shell.as_str() {
        "bash" => clap_complete::Shell::Bash,
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        "powershell" => clap_complete::Shell::PowerShell,
        "elvish" => clap_complete::Shell::Elvish,
        _ => {
            eprintln!(
                "不支持的 shell: {}. 支持的 shell: bash, zsh, fish, powershell, elvish",
                shell
            );
            std::process::exit(1);
        }
    };

    clap_complete::generate(shell_type, &mut cmd, "pkg-checker", &mut std::io::stdout());
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 处理 shell 补全生成
    if let Some(shell) = cli.completion {
        generate_completion(shell);
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
        println!("\n{}", "检测到以下包有更新:".yellow().bold());

        // 显示稳定版本更新
        if !stable_updates.is_empty() {
            println!("{}", "稳定版本更新:".green().bold());
            for package in &stable_updates {
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
            for package in &prerelease_updates {
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
        if Confirm::new()
            .with_prompt("是否要更新这些包？")
            .default(true)
            .interact()?
        {
            // 如果有预发布版本，询问是否包含
            let mut packages_to_update = stable_updates.clone();
            if !prerelease_updates.is_empty()
                && Confirm::new()
                    .with_prompt("是否包含预发布版本更新？")
                    .default(false)
                    .interact()?
            {
                packages_to_update.extend(prerelease_updates);
            }

            // 让用户选择要更新的包
            let package_names: Vec<String> =
                packages_to_update.iter().map(|p| p.name.clone()).collect();

            let selections = MultiSelect::new()
                .with_prompt("选择要更新的包（使用空格选择，回车确认）")
                .items(&package_names)
                .interact()?;

            if !selections.is_empty() {
                println!("\n{}", "开始更新选中的包...".blue().bold());

                let mut success_count = 0;
                let mut fail_count = 0;
                let total_packages = selections.len();

                // 创建整体进度条
                let main_pb = ProgressBar::new(total_packages as u64);
                main_pb.set_style(
                    ProgressStyle::default_bar()
                        .template("{bar:40.green/blue} {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("█▉▊▋▌▍▎▏  "),
                );
                main_pb.set_message("正在更新包...");

                for (i, &index) in selections.iter().enumerate() {
                    let package_name = &package_names[index];

                    // 更新整体进度条消息
                    main_pb.set_message(format!(
                        "正在更新 {} ({}/{})",
                        package_name,
                        i + 1,
                        total_packages
                    ));

                    // 找到对应的包信息以获取目标版本
                    let target_version = packages_to_update
                        .iter()
                        .find(|p| p.name == *package_name)
                        .and_then(|p| p.latest_version.as_ref())
                        .map(|v| v.as_str());

                    match update_package(package_name, target_version).await {
                        Ok(true) => {
                            success_count += 1;
                            main_pb.println(format!("✅ {} 更新成功", package_name.green()));
                        }
                        Ok(false) => {
                            fail_count += 1;
                            main_pb.println(format!("❌ {} 更新失败", package_name.red()));
                        }
                        Err(e) => {
                            main_pb.println(format!("❌ {} 更新出错: {}", package_name.red(), e));
                            fail_count += 1;
                        }
                    }

                    // 更新进度条
                    main_pb.inc(1);
                }

                // 完成整体进度条
                main_pb.finish_with_message("所有包更新完成！");

                println!("\n{}", "更新完成！".green().bold());
                println!("成功: {} 个包", success_count.to_string().green());
                if fail_count > 0 {
                    println!("失败: {} 个包", fail_count.to_string().red());
                }
            } else {
                println!("{}", "未选择任何包进行更新".yellow());
            }
        } else {
            println!("{}", "跳过更新".yellow());
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
