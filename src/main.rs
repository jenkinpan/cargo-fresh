use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use colored::*;

mod cli;
mod display;
mod locale;
mod models;
mod package;
mod updater;

use cli::{Cli, Commands, OutputFormat};
use display::{
    print_results, print_update_selection, print_update_summary, set_json_mode, status,
    status_dim, status_err, status_warn,
};
use locale::detect_language;
use models::{
    JsonReport, JsonResult, JsonSkipped, JsonSummary, JsonUpdateCandidate, PackageInfo,
    PackageSource, UpdateResult,
};
use package::{
    check_package_updates, exclude_packages, filter_packages, get_installed_packages,
    is_stable_version,
};
use updater::update_package;

/// 退出码契约（在 README 同步文档化）：
///
/// | 码  | 含义                                            |
/// |-----|-------------------------------------------------|
/// | 0   | 无更新候选；或所有选中包更新成功                |
/// | 1   | 有更新候选但本次未应用（JSON 模式无 --batch；或 --no-interactive 没选中包） |
/// | 2   | 至少一个包更新失败                              |
/// | 130 | 用户按 Ctrl-C 中断                              |
/// | 其他| clap 用法错误等由 clap / anyhow 直接返回的标准码 |
const EXIT_OK: i32 = 0;
const EXIT_UPDATES_AVAILABLE: i32 = 1;
const EXIT_FAILED: i32 = 2;
const EXIT_ABORTED: i32 = 130;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cli = if args.get(1) == Some(&"fresh".to_string()) {
        Cli::parse_from(args.into_iter().skip(1))
    } else {
        Cli::parse()
    };

    // JSON 模式：禁用所有 status*/print_*/dialoguer 输出，结尾统一打一行 JSON。
    let json_mode = cli.format == OutputFormat::Json;
    if json_mode {
        set_json_mode(true);
    }

    let language = detect_language();

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let cancel = cancel.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                cancel.store(true, Ordering::SeqCst);
            }
        });
    }

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

    let run_start = std::time::Instant::now();
    status("Checking", language.get_text("checking_packages"));

    let mut packages = get_installed_packages().await?;

    if packages.is_empty() {
        status_warn("Note", language.get_text("no_packages_found"));
        if json_mode {
            emit_empty_report(&cli, run_start);
        }
        return Ok(());
    }

    if let Some(filter_pattern) = &cli.filter {
        filter_packages(&mut packages, filter_pattern)?;
    }
    if !cli.exclude.is_empty() {
        exclude_packages(&mut packages, &cli.exclude)?;
    }
    if (cli.filter.is_some() || !cli.exclude.is_empty()) && packages.is_empty() {
        status_warn("Note", language.get_text("no_packages_found"));
        if json_mode {
            emit_empty_report(&cli, run_start);
        }
        return Ok(());
    }

    status(
        "Found",
        &language
            .get_text("found_packages")
            .replace("{}", &packages.len().to_string()),
    );

    check_package_updates(
        &mut packages,
        cli.verbose,
        cli.include_prerelease,
        cli.registry_url.clone(),
    )
    .await?;

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

    let prerelease_updates: Vec<&PackageInfo> = packages
        .iter()
        .filter(|p| p.has_update() && p.is_prerelease())
        .collect();

    let mut all_updates = stable_updates.clone();
    all_updates.extend(prerelease_updates.clone());

    if all_updates.is_empty() {
        status("Finished", language.get_text("all_up_to_date"));
        if json_mode {
            emit_report(&cli, &packages, &[], &[], false, run_start);
        }
        return Ok(());
    }

    print_results(&packages, cli.updates_only, language);

    // 选包：
    // - JSON + --batch：选所有更新候选
    // - JSON 无 --batch：不选任何（只检查，退出码 1 表示"有可更新"）
    // - human --batch：选所有
    // - human 交互：dialoguer 多选
    // - human --no-interactive：不选
    let selections: Vec<usize> = if cli.batch {
        (0..all_updates.len()).collect()
    } else if json_mode || cli.no_interactive {
        Vec::new()
    } else {
        print_update_selection(&stable_updates, &prerelease_updates, language)?
    };

    let mut update_results: Vec<UpdateResult> = Vec::new();
    let mut aborted = false;

    if !selections.is_empty() {
        if !json_mode {
            println!();
            if cli.dry_run {
                status("Dry run", language.get_text("dry_run_summary"));
            } else {
                status("Updating", language.get_text("starting_update"));
            }
        }

        let mut all_packages_to_update = stable_updates.clone();
        all_packages_to_update.extend(prerelease_updates.clone());
        let total_packages = selections.len();
        let mut success_count = 0;
        let mut fail_count = 0;
        let mut aborted_at: Option<usize> = None;

        for (i, &index) in selections.iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                aborted_at = Some(i);
                break;
            }
            let package_name = &all_packages_to_update[index].name;
            let selected_pkg = all_packages_to_update
                .iter()
                .find(|p| p.name == *package_name);
            let target_version = selected_pkg
                .and_then(|p| p.latest_version.as_ref())
                .map(|v| v.as_str());
            let source = selected_pkg
                .map(|p| p.source.clone())
                .unwrap_or(PackageSource::Crates);

            if total_packages > 1 {
                status_dim(
                    "Package",
                    &format!("{}/{} {}", i + 1, total_packages, package_name.cyan()),
                );
            }

            match update_package(package_name, target_version, &source, cli.dry_run).await {
                Ok(result) => {
                    if result.success {
                        success_count += 1;
                    } else {
                        fail_count += 1;
                    }
                    update_results.push(result);
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

        let total_duration = run_start.elapsed();
        let duration_seconds = total_duration.as_secs();
        let duration_millis = total_duration.as_millis();

        print_update_summary(&update_results, language);

        if let Some(done) = aborted_at {
            aborted = true;
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
        } else {
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
        }
    } else {
        status_dim("Note", language.get_text("no_packages_selected"));
    }

    if json_mode {
        emit_report(
            &cli,
            &packages,
            &all_updates,
            &update_results,
            aborted,
            run_start,
        );
    }

    // 计算退出码
    let any_failed = update_results.iter().any(|r| !r.success);
    let updates_available = !all_updates.is_empty();
    let applied_any = !update_results.is_empty();

    let code = if aborted {
        EXIT_ABORTED
    } else if any_failed {
        EXIT_FAILED
    } else if updates_available && !applied_any {
        EXIT_UPDATES_AVAILABLE
    } else {
        EXIT_OK
    };

    if code != EXIT_OK {
        std::process::exit(code);
    }
    Ok(())
}

/// 提前退出（没有任何包）时打一份空 JSON 报告，保持 stdout 始终单行可解析。
fn emit_empty_report(cli: &Cli, start: std::time::Instant) {
    let report = JsonReport {
        schema_version: 1,
        format: "cargo-fresh-v1",
        include_prerelease: cli.include_prerelease,
        dry_run: cli.dry_run,
        registry_url: cli.registry_url.as_deref(),
        updates_available: vec![],
        fresh: vec![],
        skipped: vec![],
        results: vec![],
        summary: JsonSummary {
            checked: 0,
            available: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            duration_ms: start.elapsed().as_millis(),
        },
        aborted: false,
    };
    print_json(&report);
}

fn emit_report(
    cli: &Cli,
    packages: &[PackageInfo],
    all_updates: &[&PackageInfo],
    update_results: &[UpdateResult],
    aborted: bool,
    start: std::time::Instant,
) {
    let updates_available: Vec<JsonUpdateCandidate> = all_updates
        .iter()
        .filter_map(|p| {
            p.latest_version.as_deref().map(|latest| JsonUpdateCandidate {
                name: p.name.as_str(),
                current: p.current_version.as_deref(),
                latest,
                source: p.source.kind_str(),
                prerelease: p.is_prerelease(),
            })
        })
        .collect();

    let fresh: Vec<&str> = packages
        .iter()
        .filter(|p| !p.has_update() && p.source.is_crates())
        .map(|p| p.name.as_str())
        .collect();

    // 跳过的包：git / path 源没有版本检查
    let skipped: Vec<JsonSkipped> = packages
        .iter()
        .filter(|p| !p.source.is_crates())
        .map(|p| JsonSkipped {
            name: p.name.as_str(),
            source: p.source.kind_str(),
            reason: "non-crates source: version check skipped",
        })
        .collect();

    let results: Vec<JsonResult> = update_results
        .iter()
        .map(|r| JsonResult {
            name: r.package_name.as_str(),
            old_version: r.old_version.as_deref(),
            new_version: r.new_version.as_deref(),
            success: r.success,
        })
        .collect();

    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.iter().filter(|r| !r.success).count();

    let summary = JsonSummary {
        checked: packages.len(),
        available: updates_available.len(),
        succeeded,
        failed,
        skipped: skipped.len(),
        duration_ms: start.elapsed().as_millis(),
    };

    let report = JsonReport {
        schema_version: 1,
        format: "cargo-fresh-v1",
        include_prerelease: cli.include_prerelease,
        dry_run: cli.dry_run,
        registry_url: cli.registry_url.as_deref(),
        updates_available,
        fresh,
        skipped,
        results,
        summary,
        aborted,
    };

    print_json(&report);
}

fn print_json(report: &JsonReport) {
    match serde_json::to_string(report) {
        Ok(s) => println!("{}", s),
        Err(e) => {
            eprintln!("{{\"schema_version\":1,\"error\":\"failed to serialize report: {}\"}}", e);
        }
    }
}
