use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use colored::*;

use cargo_fresh::cli::{Cli, Commands, OutputFormat};
use cargo_fresh::display::{
    print_results, print_update_selection, print_update_summary, set_json_mode, status,
    status_dim, status_err, status_warn,
};
use cargo_fresh::locale::detect_language;
use cargo_fresh::models::{
    JsonCheckError, JsonReport, JsonResult, JsonSkipped, JsonSummary, JsonUpdateCandidate,
    PackageInfo, PackageSource, UpdateResult,
};
use cargo_fresh::package::{
    check_package_updates, exclude_packages, filter_packages, get_installed_packages,
    is_stable_version,
};
use cargo_fresh::updater::update_package;

/// Outcome of a single package update, ready for the orchestrator to fold
/// into `success_count` / `fail_count` / `aborted_at` / `update_results`.
enum SlotOutcome {
    Success(UpdateResult),
    Failed(UpdateResult),
    Aborted,
    Error(String, anyhow::Error),
}

#[allow(clippy::too_many_arguments)]
async fn run_one_update(
    package_name: String,
    target_version: Option<String>,
    source: PackageSource,
    install_opts: Option<cargo_fresh::models::InstallOpts>,
    dry_run: bool,
    verbose: bool,
    cancel: Arc<AtomicBool>,
    row: Option<(indicatif::ProgressBar, usize)>,
) -> SlotOutcome {
    let row_for_finalize = row.clone();
    let target = target_version.as_deref();
    let opts_ref = install_opts.as_ref();

    match update_package(
        &package_name,
        target,
        &source,
        opts_ref,
        dry_run,
        verbose,
        cancel,
        row,
    )
    .await
    {
        Ok(Some(result)) => {
            if let Some((pb, w)) = &row_for_finalize {
                if result.success {
                    cargo_fresh::updater::finalize_installed(pb, *w);
                } else {
                    cargo_fresh::updater::finalize_failed(pb, *w, "");
                }
            }
            if result.success {
                SlotOutcome::Success(result)
            } else {
                SlotOutcome::Failed(result)
            }
        }
        Ok(None) => {
            if let Some((pb, w)) = &row_for_finalize {
                cargo_fresh::updater::finalize_aborted(pb, *w);
            }
            SlotOutcome::Aborted
        }
        Err(e) => {
            if let Some((pb, w)) = &row_for_finalize {
                cargo_fresh::updater::finalize_failed(pb, *w, &e.to_string());
            }
            SlotOutcome::Error(package_name, e)
        }
    }
}

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
async fn main() {
    let exit_code = match run().await {
        Ok(code) => code,
        Err(err) => {
            // anyhow 默认会把整条 chain 打到 stderr，我们额外补一条 "Hint" 行
            // 给出可执行建议（如果 errors::hint_for 能识别这条错误）。
            // 用 anstream::eprintln 而非 status_err：anstream 已经处理过 NO_COLOR/TTY；
            // hint 用 status_dim 走和 spinner 同一条 stderr 通道。
            anstream::eprintln!("error: {err:?}");
            if let Some(hint) = cargo_fresh::errors::hint_for(&err) {
                if !cargo_fresh::display::is_json_mode() {
                    status_dim("Hint", hint);
                }
            }
            std::process::exit(EXIT_FAILED);
        }
    };
    std::process::exit(exit_code);
}

async fn run() -> Result<i32> {
    let args: Vec<String> = std::env::args().collect();
    let cli = if args.get(1) == Some(&"fresh".to_string()) {
        Cli::parse_from(args.into_iter().skip(1))
    } else {
        Cli::parse()
    };

    // 颜色决策权交给 anstream：它读取 NO_COLOR / CLICOLOR[_FORCE] / TERM / TTY
    // 一次得出 ColorChoice，我们把这个决定下发给 colored（让 `.green().bold()` 这套
    // 生成 ANSI，但是否实际写到终端最终还是 anstream::eprintln! 在每次调用时再裁剪）。
    // status 全部走 stderr，因此用 stderr 的 choice 校准；JSON 模式独立走 anstream::stdout。
    let stderr_choice = anstream::AutoStream::choice(&std::io::stderr());
    colored::control::set_override(stderr_choice != anstream::ColorChoice::Never);

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
            // First Ctrl-C: set flag, in-flight tasks drain naturally.
            // Second Ctrl-C: force-exit. TempDir Drop handles tempfile cleanup;
            // atomic-rename guarantees no half-installed binaries in ~/.cargo/bin.
            let mut presses: u8 = 0;
            loop {
                if tokio::signal::ctrl_c().await.is_err() {
                    return;
                }
                presses += 1;
                if presses == 1 {
                    cancel.store(true, Ordering::SeqCst);
                    status_warn("Aborting", "Ctrl-C again to force exit");
                } else {
                    std::process::exit(130);
                }
            }
        });
    }

    if let Some(command) = cli.command {
        match command {
            Commands::Completion { shell, cargo_fresh, install } => {
                if install {
                    match Cli::install_completion(&shell, cargo_fresh, language) {
                        Ok(cargo_fresh::cli::InstallOutcome::Written(path)) => {
                            status(
                                "Installed",
                                &language
                                    .get_text("completion_installed_path")
                                    .replace("{}", &path.display().to_string()),
                            );
                        }
                        Ok(cargo_fresh::cli::InstallOutcome::Skipped(path)) => {
                            status_warn(
                                "Skipped",
                                &language
                                    .get_text("completion_path_exists")
                                    .replace("{}", &path.display().to_string()),
                            );
                            return Ok(EXIT_UPDATES_AVAILABLE);
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    }
                    return Ok(EXIT_OK);
                }
                if cargo_fresh {
                    Cli::generate_cargo_fresh_completion(shell.clone());
                } else {
                    Cli::generate_completion(shell.clone());
                }
                Cli::maybe_hint_fish_install(&shell, cargo_fresh);
                return Ok(EXIT_OK);
            }
            Commands::Man => {
                Cli::generate_man()?;
                return Ok(EXIT_OK);
            }
        }
    }

    let run_start = std::time::Instant::now();
    status("Checking", language.get_text("checking_packages"));

    let mut packages = get_installed_packages().await?;

    if packages.is_empty() {
        status_warn("Note", language.get_text("no_packages_found"));
        if json_mode {
            emit_report(&cli, &[], &[], &[], false, run_start, 0);
        }
        return Ok(EXIT_OK);
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
            emit_report(&cli, &[], &[], &[], false, run_start, 0);
        }
        return Ok(EXIT_OK);
    }

    status(
        "Found",
        &language
            .get_text("found_packages")
            .replace("{}", &packages.len().to_string()),
    );

    let no_fallback = cargo_fresh::package::cargo_search_fallback_disabled(cli.no_cargo_search_fallback);
    check_package_updates(
        &mut packages,
        cli.verbose,
        cli.include_prerelease,
        cli.registry_url.clone(),
        no_fallback,
    )
    .await?;

    // --check-prebuilt:对更新候选并发跑 HEAD 探测,提前标出"会拿预编译产物(快)"
    // 还是"会回退到 cargo install 从源码构建(慢)"。和真正的 update 路径用同一份
    // resolve + HEAD 逻辑,结果保持一致。
    if cli.check_prebuilt {
        cargo_fresh::downloader::probe::annotate_updates(&mut packages).await;
    }

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
            emit_report(&cli, &packages, &[], &[], false, run_start, 0);
        }
        return Ok(EXIT_OK);
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
            anstream::eprintln!();
            if cli.dry_run {
                status("Dry run", language.get_text("dry_run_summary"));
            } else {
                status("Updating", language.get_text("starting_update"));
            }
        }

        let mut all_packages_to_update = stable_updates.clone();
        all_packages_to_update.extend(prerelease_updates.clone());
        let mut success_count = 0;
        let mut fail_count = 0;
        let mut aborted_at: Option<usize> = None;

        // 预注册所有选中包成 rustup 风格的对齐行 (pending 状态), 把每行的
        // ProgressBar + 对齐宽度递给 update_package, 它在 phase 切换里更新这一行
        let selected_names: Vec<String> = selections
            .iter()
            .map(|&i| all_packages_to_update[i].name.clone())
            .collect();
        let plan_arc = (!cli.dry_run && !json_mode)
            .then(|| Arc::new(cargo_fresh::updater::UpdatePlan::new(&selected_names)));

        let cap = if cli.jobs == 0 {
            selections.len().max(1)
        } else {
            cli.jobs as usize
        };
        let sem = Arc::new(Semaphore::new(cap));
        let mut set: JoinSet<(usize, SlotOutcome)> = JoinSet::new();

        for (i, &index) in selections.iter().enumerate() {
            // Pre-flight cancel check — if user Ctrl-C'd before scheduling,
            // mark aborted and stop spawning.
            if cancel.load(Ordering::SeqCst) {
                aborted_at = Some(i);
                if let Some(p) = plan_arc.as_ref() {
                    cargo_fresh::updater::finalize_aborted(&p.row(i), p.name_width());
                }
                break;
            }

            let package_name = all_packages_to_update[index].name.clone();
            let selected_pkg = all_packages_to_update
                .iter()
                .find(|p| p.name == package_name);
            let target_version = selected_pkg
                .and_then(|p| p.latest_version.as_ref())
                .cloned();
            let source = selected_pkg
                .map(|p| p.source.clone())
                .unwrap_or(PackageSource::Crates);
            let install_opts = selected_pkg.and_then(|p| p.install_opts.clone());

            let row = plan_arc
                .as_ref()
                .map(|p| (p.row(i), p.name_width()));

            // acquire_owned BEFORE spawn — this is what bounds concurrency.
            let permit = match sem.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };
            let cancel_task = cancel.clone();
            let dry_run = cli.dry_run;
            let verbose = cli.verbose;

            set.spawn(async move {
                let _permit = permit; // released on task drop
                let outcome = run_one_update(
                    package_name,
                    target_version,
                    source,
                    install_opts,
                    dry_run,
                    verbose,
                    cancel_task,
                    row,
                )
                .await;
                (i, outcome)
            });
        }

        // Drain in completion order; sort to input order before folding so
        // counts and update_results match on-screen row order.
        let mut indexed: Vec<(usize, SlotOutcome)> = Vec::with_capacity(selections.len());
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok(pair) => indexed.push(pair),
                Err(e) => {
                    // Tokio JoinError — task panicked. Log and continue.
                    status_err("Error", &format!("task panicked: {e}"));
                }
            }
        }
        indexed.sort_by_key(|(i, _)| *i);

        for (i, outcome) in indexed {
            match outcome {
                SlotOutcome::Success(result) => {
                    success_count += 1;
                    update_results.push(result);
                }
                SlotOutcome::Failed(result) => {
                    fail_count += 1;
                    update_results.push(result);
                }
                SlotOutcome::Aborted => {
                    aborted_at.get_or_insert(i);
                }
                SlotOutcome::Error(name, e) => {
                    if plan_arc.is_none() {
                        status_err(
                            "Error",
                            &language.format_text(
                                "package_error",
                                &[
                                    ("name", &name.red().to_string()),
                                    ("error", &e.to_string()),
                                ],
                            ),
                        );
                    }
                    fail_count += 1;
                    update_results.push(UpdateResult::new(name, None, None, false));
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
                        ("total", &selections.len().to_string()),
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
            selections.len(),
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

    Ok(code)
}

/// 纯函数：把整次运行的快照组装成 JSON 报告结构。
/// 不做 I/O，便于单元测试；`emit_report` 负责真正写 stdout。
fn build_report<'a>(
    cli: &'a Cli,
    packages: &'a [PackageInfo],
    all_updates: &[&'a PackageInfo],
    update_results: &'a [UpdateResult],
    aborted: bool,
    start: std::time::Instant,
    selected: usize,
) -> JsonReport<'a> {
    let updates_available: Vec<JsonUpdateCandidate> = all_updates
        .iter()
        .filter_map(|p| {
            p.latest_version.as_deref().map(|latest| JsonUpdateCandidate {
                name: p.name.as_str(),
                current: p.current_version.as_deref(),
                latest,
                source: p.source.kind_str(),
                prerelease: p.is_prerelease(),
                prebuilt: p.prebuilt.map(|k| k.kind_str()),
            })
        })
        .collect();

    let fresh: Vec<&str> = packages
        .iter()
        .filter(|p| !p.has_update() && p.source.is_crates() && p.check_error.is_none())
        .map(|p| p.name.as_str())
        .collect();

    let skipped: Vec<JsonSkipped> = packages
        .iter()
        .filter(|p| !p.source.is_crates())
        .map(|p| JsonSkipped {
            name: p.name.as_str(),
            source: p.source.kind_str(),
            reason_code: p.source.skip_reason_code(),
            reason: "non-crates source: version check skipped",
        })
        .collect();

    let version_check_errors: Vec<JsonCheckError> = packages
        .iter()
        .filter_map(|p| {
            p.check_error.as_ref().map(|e| JsonCheckError {
                name: p.name.as_str(),
                kind: e.kind.kind_str(),
                error: e.message.as_str(),
            })
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
        selected,
        attempted: results.len(),
        succeeded,
        failed,
        skipped: skipped.len(),
        check_errors: version_check_errors.len(),
        duration_ms: start.elapsed().as_millis(),
    };

    JsonReport {
        schema_version: 2,
        format: "cargo-fresh-v1",
        include_prerelease: cli.include_prerelease,
        dry_run: cli.dry_run,
        registry_url: cli.registry_url.as_deref(),
        updates_available,
        fresh,
        skipped,
        version_check_errors,
        results,
        summary,
        aborted,
    }
}

fn emit_report(
    cli: &Cli,
    packages: &[PackageInfo],
    all_updates: &[&PackageInfo],
    update_results: &[UpdateResult],
    aborted: bool,
    start: std::time::Instant,
    selected: usize,
) {
    print_json(&build_report(
        cli,
        packages,
        all_updates,
        update_results,
        aborted,
        start,
        selected,
    ));
}

fn print_json(report: &JsonReport) {
    // JSON 报告永远不需要颜色，但仍走 anstream::println! 以保持 stdout 通道一致
    match serde_json::to_string(report) {
        Ok(s) => anstream::println!("{}", s),
        Err(e) => {
            anstream::eprintln!(
                "{{\"schema_version\":2,\"error\":\"failed to serialize report: {}\"}}", e
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use cargo_fresh::models::{PackageInfo, PackageSource};

    fn empty_cli() -> Cli {
        Cli::parse_from(["cargo-fresh"])
    }

    #[test]
    fn build_report_counts_packages_and_sets_format() {
        let cli = empty_cli();
        let packages = vec![
            PackageInfo::with_source("ripgrep".into(), Some("14.1.1".into()), PackageSource::Crates),
        ];
        let report = build_report(&cli, &packages, &[], &[], false, std::time::Instant::now(), 0);
        assert_eq!(report.schema_version, 2);
        assert_eq!(report.format, "cargo-fresh-v1");
        assert_eq!(report.summary.checked, 1);
        assert_eq!(report.fresh, vec!["ripgrep"]);
    }

    #[test]
    fn build_report_sets_skip_reason_code() {
        let cli = empty_cli();
        let packages = vec![PackageInfo::with_source(
            "my-tool".into(),
            Some("0.1.0".into()),
            PackageSource::Git { url: "u".into(), rev: None },
        )];
        let report = build_report(&cli, &packages, &[], &[], false, std::time::Instant::now(), 0);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason_code, "git_source");
    }

    #[test]
    fn build_report_excludes_check_error_packages_from_fresh() {
        use cargo_fresh::models::{CheckError, CheckErrorKind};

        let cli = empty_cli();
        let mut errored = PackageInfo::with_source(
            "bat".into(),
            Some("0.24.0".into()),
            PackageSource::Crates,
        );
        errored.check_error = Some(CheckError {
            kind: CheckErrorKind::Unavailable,
            message: "sparse index HTTP 503".into(),
        });
        let fresh_pkg = PackageInfo::with_source(
            "ripgrep".into(),
            Some("14.1.1".into()),
            PackageSource::Crates,
        );
        let packages = vec![errored, fresh_pkg];

        let report = build_report(&cli, &packages, &[], &[], false, std::time::Instant::now(), 0);

        assert_eq!(report.fresh, vec!["ripgrep"]);
        assert_eq!(report.version_check_errors.len(), 1);
        assert_eq!(report.version_check_errors[0].name, "bat");
        assert_eq!(report.version_check_errors[0].kind, "unavailable");
        assert_eq!(report.summary.check_errors, 1);
    }

    #[test]
    fn build_report_maps_prebuilt_to_json() {
        // --check-prebuilt 探测出的 PrebuiltAvailability 必须落到 updates_available[].prebuilt
        use cargo_fresh::models::PrebuiltAvailability;
        let cli = empty_cli();
        let mut pkg = PackageInfo::with_source(
            "cargo-deny".into(),
            Some("0.19.6".into()),
            PackageSource::Crates,
        );
        pkg.latest_version = Some("0.19.7".into());
        pkg.prebuilt = Some(PrebuiltAvailability::Source);
        let packages = vec![pkg];
        let all_updates: Vec<&PackageInfo> = packages.iter().collect();
        let report = build_report(
            &cli,
            &packages,
            &all_updates,
            &[],
            false,
            std::time::Instant::now(),
            0,
        );
        assert_eq!(report.updates_available.len(), 1);
        assert_eq!(report.updates_available[0].prebuilt, Some("source"));
    }

    #[test]
    fn build_report_prebuilt_is_null_when_not_probed() {
        // 没跑 --check-prebuilt 时 pkg.prebuilt 为 None,JSON 里应是 null
        let cli = empty_cli();
        let mut pkg = PackageInfo::with_source(
            "ripgrep".into(),
            Some("14.1.0".into()),
            PackageSource::Crates,
        );
        pkg.latest_version = Some("14.1.1".into());
        let packages = vec![pkg];
        let all_updates: Vec<&PackageInfo> = packages.iter().collect();
        let report = build_report(
            &cli,
            &packages,
            &all_updates,
            &[],
            false,
            std::time::Instant::now(),
            0,
        );
        assert_eq!(report.updates_available[0].prebuilt, None);
    }

    #[test]
    fn build_report_summary_has_selection_counts() {
        let cli = empty_cli();
        let report = build_report(
            &cli,
            &[],
            &[],
            &[],
            false,
            std::time::Instant::now(),
            3,
        );
        assert_eq!(report.summary.selected, 3);
        assert_eq!(report.summary.attempted, 0);
    }
}
