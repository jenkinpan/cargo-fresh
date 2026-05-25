use std::sync::atomic::{AtomicBool, Ordering};

use colored::*;
use dialoguer::{Confirm, MultiSelect};
use indicatif::ProgressBar;

use crate::locale::Language;
use crate::models::{BinstallKind, PackageInfo, UpdateResult};

/// JSON mode 开关：在 main 早期被设置一次，之后所有 status* / print_* /
/// dialoguer 调用都自动 no-op，避免污染 JSON 输出。
static JSON_MODE: AtomicBool = AtomicBool::new(false);

/// 由 main 在解析 CLI 后调用一次。
pub fn set_json_mode(enabled: bool) {
    JSON_MODE.store(enabled, Ordering::SeqCst);
}

pub fn is_json_mode() -> bool {
    JSON_MODE.load(Ordering::SeqCst)
}

/// Cargo 风格状态行的右对齐宽度。
///
/// 与 `cargo build` 输出对齐——12 字符容得下 `Compiling` / `Installing` / `Finished` 等
/// 主要动词。所有 status 系列函数共用这个宽度，保持视觉上整齐成列。
const STATUS_WIDTH: usize = 12;

/// status 行的语义色：决定 verb 是绿(Ok)/黄(Warn)/红(Err)/灰(Dim)。
///
/// 抽出来让 `format_status_line` 成为纯函数,既消除四个 `status*` /
/// 四个 `pb_status*` 函数里的重复格式串,也让 snapshot 测试能直接锁住
/// "verb 名 + 12 字宽对齐 + 颜色选择"这套外观契约。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusStyle {
    Ok,
    Warn,
    Err,
    Dim,
}

/// 纯函数:按 cargo 风格组装单行 status——12 字右对齐 verb + 空格 + 描述。
///
/// 返回 String,不做 I/O,不查 JSON_MODE。是 verb 字典对外契约的唯一
/// render 路径——`tests/cli_snapshots.rs` 直接对它的输出做 snapshot,
/// 任何对 verb 名/宽度/颜色的改动都会在 PR diff 上以 `.snap` 变化形式
/// 出现。所有 `status*` / `pb_status*` 都委托给这一处。
///
/// 颜色码不计入宽度——必须先 pad 再上色,否则 ANSI 序列被算进宽度导致错位。
pub fn format_status_line(verb: &str, msg: &str, style: StatusStyle) -> String {
    let padded = format!("{:>w$}", verb, w = STATUS_WIDTH);
    let colored_verb = match style {
        StatusStyle::Ok => padded.green().bold().to_string(),
        StatusStyle::Warn => padded.yellow().bold().to_string(),
        StatusStyle::Err => padded.red().bold().to_string(),
        StatusStyle::Dim => padded.dimmed().to_string(),
    };
    format!("{} {}", colored_verb, msg)
}

/// 用 cargo 风格输出一行状态："{右对齐12字符绿色加粗动词} {描述}"。
///
/// 颜色变体：`status_warn` 黄、`status_err` 红、`status_dim` 灰（用于次要信息）。
pub fn status(verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    anstream::eprintln!("{}", format_status_line(verb, msg, StatusStyle::Ok));
}

pub fn status_warn(verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    anstream::eprintln!("{}", format_status_line(verb, msg, StatusStyle::Warn));
}

pub fn status_err(verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    anstream::eprintln!("{}", format_status_line(verb, msg, StatusStyle::Err));
}

pub fn status_dim(verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    anstream::eprintln!("{}", format_status_line(verb, msg, StatusStyle::Dim));
}

/// 同 `status`，但把输出送到指定的 ProgressBar（避免与活动进度条冲突）。
pub fn pb_status(pb: &ProgressBar, verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    pb.println(format_status_line(verb, msg, StatusStyle::Ok));
}

pub fn pb_status_warn(pb: &ProgressBar, verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    pb.println(format_status_line(verb, msg, StatusStyle::Warn));
}

pub fn pb_status_err(pb: &ProgressBar, verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    pb.println(format_status_line(verb, msg, StatusStyle::Err));
}

pub fn pb_status_dim(pb: &ProgressBar, verb: &str, msg: &str) {
    if is_json_mode() {
        return;
    }
    pb.println(format_status_line(verb, msg, StatusStyle::Dim));
}

/// 拼装单包的"名字 旧版本 -> 新版本 [来源]"展示字符串。
///
/// 颜色约定：包名 cyan、旧版本 red、新版本 green、来源标记 dimmed。
pub fn package_transition(package: &PackageInfo, language: Language) -> String {
    let current = package
        .current_version
        .as_deref()
        .unwrap_or(language.get_text("unknown"));
    let latest = package
        .latest_version
        .as_deref()
        .unwrap_or(language.get_text("unknown"));
    let marker = package.source.marker();
    let suffix = if marker.is_empty() {
        String::new()
    } else {
        format!(" {}", marker.dimmed())
    };
    let binstall_suffix = match package.binstall_kind {
        Some(kind) => format!(" {}", binstall_marker(kind)),
        None => String::new(),
    };
    format!(
        "{} {} -> {}{}{}",
        package.name.cyan(),
        current.red(),
        latest.green(),
        suffix,
        binstall_suffix
    )
}

/// 给 binstall 预检标记上色:预编译绿(好消息)、源码构建黄(预警:这次升级
/// 会慢)、无法判别 dim。`--check-binstall` 时挂在 `Updating` 行尾。
fn binstall_marker(kind: BinstallKind) -> String {
    match kind {
        BinstallKind::Prebuilt => kind.marker().green().to_string(),
        BinstallKind::SourceBuild => kind.marker().yellow().to_string(),
        BinstallKind::Unknown => kind.marker().dimmed().to_string(),
    }
}

/// 把 (old, new) 渲染成 "old -> new"（带颜色），或 "old (unchanged)"。
pub fn format_version_info(
    old: &Option<String>,
    new: &Option<String>,
    language: Language,
) -> String {
    match (old, new) {
        (Some(old), Some(new)) if old != new => {
            format!("{} -> {}", old.red(), new.green())
        }
        (Some(old), Some(_)) => {
            format!(
                "{} ({})",
                old.yellow(),
                language.get_text("version_unchanged").dimmed()
            )
        }
        (Some(old), None) => {
            format!("{} -> {}", old.red(), language.get_text("unknown_version").dimmed())
        }
        (None, Some(new)) => {
            format!("{} -> {}", language.get_text("unknown_version").dimmed(), new.green())
        }
        _ => language.get_text("version_info_unknown").dimmed().to_string(),
    }
}

fn package_with_source(package: &PackageInfo) -> String {
    let marker = package.source.marker();
    if marker.is_empty() {
        package.name.cyan().to_string()
    } else {
        format!("{} {}", package.name.cyan(), marker.dimmed())
    }
}

pub fn print_results(packages: &[PackageInfo], updates_only: bool, language: Language) {
    if is_json_mode() {
        return;
    }
    let mut has_updates = false;
    let mut fresh_count = 0;

    for package in packages {
        if updates_only && !package.has_update() {
            continue;
        }

        if package.has_update() {
            has_updates = true;
            status("Updating", &package_transition(package, language));
        } else if !updates_only {
            let version = package.current_version.as_deref().unwrap_or("?");
            status_dim(
                "Fresh",
                &format!("{} {}", package_with_source(package), version.dimmed()),
            );
            fresh_count += 1;
        } else {
            fresh_count += 1;
        }
    }

    if updates_only && !has_updates {
        status("Finished", language.get_text("all_up_to_date"));
    } else if !has_updates && fresh_count > 0 {
        // 列了所有包但没有更新时给个汇总尾行
        status("Finished", language.get_text("all_up_to_date"));
    }
}

pub fn print_update_summary(update_results: &[UpdateResult], language: Language) {
    if update_results.is_empty() {
        return;
    }

    let mut success_updates = Vec::new();
    let mut failed_updates = Vec::new();

    for result in update_results {
        if result.success {
            success_updates.push(result);
        } else {
            failed_updates.push(result);
        }
    }

    if is_json_mode() {
        return;
    }
    anstream::eprintln!();
    anstream::eprintln!("{}", language.get_text("update_summary").bold());

    if !success_updates.is_empty() {
        for result in &success_updates {
            status(
                "Updated",
                &format!(
                    "{} {}",
                    result.package_name.cyan(),
                    format_version_info(&result.old_version, &result.new_version, language)
                ),
            );
        }
    }

    if !failed_updates.is_empty() {
        for result in &failed_updates {
            let detail = match &result.old_version {
                Some(old) => format!(
                    "{} {} ({})",
                    result.package_name.cyan(),
                    old.red(),
                    language.get_text("update_failed")
                ),
                None => format!(
                    "{} ({})",
                    result.package_name.cyan(),
                    language.get_text("update_failed")
                ),
            };
            status_err("Failed", &detail);
        }
    }
}

pub fn print_update_selection(
    stable_updates: &[&PackageInfo],
    prerelease_updates: &[&PackageInfo],
    language: Language,
) -> Result<Vec<usize>, anyhow::Error> {
    anstream::eprintln!();
    anstream::eprintln!("{}", language.get_text("updates_detected").bold());

    if !stable_updates.is_empty() {
        anstream::eprintln!("{}", language.get_text("stable_updates").dimmed());
        for package in stable_updates {
            status("Updating", &package_transition(package, language));
        }
    }

    if !prerelease_updates.is_empty() {
        anstream::eprintln!("{}", language.get_text("prerelease_updates").dimmed());
        for package in prerelease_updates {
            status_warn(
                "Prerelease",
                &format!(
                    "{} {} -> {}",
                    package.name.cyan(),
                    package
                        .current_version
                        .as_deref()
                        .unwrap_or(language.get_text("unknown"))
                        .red(),
                    package
                        .latest_version
                        .as_deref()
                        .unwrap_or(language.get_text("unknown"))
                        .yellow(),
                ),
            );
        }
    }

    anstream::eprintln!();
    let should_update = match Confirm::new()
        .with_prompt(language.get_text("update_question"))
        .default(true)
        .show_default(false)
        .interact()
    {
        Ok(choice) => choice,
        Err(e) => {
            if e.to_string().contains("not a terminal") {
                status_warn("Note", language.get_text("no_interactive_mode"));
                return Ok(vec![]);
            }
            return Err(e.into());
        }
    };

    if !should_update {
        return Ok(vec![]);
    }

    // 如果有预发布版本，询问是否包含
    let mut packages_to_update = stable_updates.to_vec();
    if !prerelease_updates.is_empty() {
        let include_prerelease = match Confirm::new()
            .with_prompt(language.get_text("include_prerelease_question"))
            .default(false)
            .show_default(false)
            .interact()
        {
            Ok(choice) => choice,
            Err(e) => {
                // 如果不是终端环境，默认不包含预发布版本
                if e.to_string().contains("not a terminal") {
                    status_warn("Note", language.get_text("no_interactive_mode"));
                    false
                } else {
                    return Err(e.into());
                }
            }
        };

        if include_prerelease {
            packages_to_update.extend(prerelease_updates);
        }
    }

    // 让用户选择要更新的包
    let package_names: Vec<String> = packages_to_update.iter().map(|p| p.name.clone()).collect();

    let selections = match MultiSelect::new()
        .with_prompt(language.get_text("select_packages"))
        .items(&package_names)
        .interact()
    {
        Ok(choices) => choices,
        Err(e) => {
            // 如果不是终端环境，默认选择所有包
            if e.to_string().contains("not a terminal") {
                anstream::eprintln!("{}", language.get_text("no_interactive_mode").yellow());
                (0..package_names.len()).collect()
            } else {
                return Err(e.into());
            }
        }
    };

    Ok(selections)
}
