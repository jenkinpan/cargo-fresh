/// Text localization functions
///
/// This module contains all the localized text strings for both
/// English and Chinese languages. It provides functions to retrieve
/// text based on language keys.
/// 获取英文文本
///
/// 根据文本键返回对应的英文文本
///
/// # Arguments
///
/// * `key` - 文本键，用于查找对应的英文文本
///
/// # Returns
///
/// 返回对应的英文文本，如果找不到则返回空字符串
pub fn get_english_text(key: &str) -> &'static str {
    match key {
        // Headers / summaries
        "checking_packages" => "for updates to globally installed packages",
        "no_packages_found" => "no installed packages match",
        "found_packages" => "{} installed package(s)",
        "all_up_to_date" => "all packages are up to date",

        // Selection prompts (dialoguer body — kept as full sentences)
        "stable_updates" => "Stable updates:",
        "prerelease_updates" => "Prerelease updates:",
        "prerelease_warning" => "prerelease",
        "update_question" => "Update these packages?",
        "include_prerelease_question" => "Include prerelease updates?",
        "select_packages" => "Select packages (space to toggle, enter to confirm)",
        "no_interactive_mode" => "non-interactive mode; skipping selection",

        // Update flow
        "starting_update" => "selected packages",
        "updating_package" => "{}",
        "package_failed" => "{}",
        "package_error" => "{name}: {error}",
        "update_completed" => "update completed",

        // Stats (used inside summary)
        "success_count" => "{} succeeded",
        "fail_count" => "{} failed",
        "no_packages_selected" => "no packages selected",
        "aborted_by_user" => "cancelled by user, {done}/{total} completed",

        // Hint text
        "update_instructions" => "To update manually: cargo install --force <name>",
        "interactive_instructions" => "Or drop --no-interactive for interactive selection",

        // Version labels
        "version_unchanged" => "version unchanged",
        "unknown_version" => "unknown",
        "version_info_unknown" => "version unknown",
        "package_has_update" => "{}",
        "current_version" => "current",
        "latest_version" => "latest",
        "package_up_to_date" => "{}",
        "version" => "version",

        // Summary headers / labels
        "update_summary" => "Update Summary",
        "successful_updates" => "Successful:",
        "failed_updates" => "Failed:",
        "update_failed" => "failed",
        "updates_detected" => "Updates available:",
        "unknown" => "unknown",

        // Updater progress text
        "updating_package_progress" => "{}",
        "current_version_label" => "currently",
        "executing_command" => "cargo",
        "retry_attempt" => "attempt {attempt} for {name}",
        "error_message" => "stderr:",
        "package_update_success" => "{} install command succeeded",
        "package_updated_version" => "{name} {old} -> {new}",
        "package_version_unchanged" => "{} version unchanged",
        "package_update_verification_failed" => "{} command succeeded but version not verified",
        "package_update_failed" => "{name} failed (exit code: {code})",
        "error_details" => "details:",
        "waiting_retry" => "waiting before retry",

        // Package module
        "cargo_install_list_failed" => "failed to execute cargo install --list",
        "checking_package" => "checking",
        "unable_to_get_latest_version" => "no latest version available",
        "check_failed" => "check failed",

        // Binstall
        "using_binstall" => "self-hosted downloader",
        "using_install_fallback" => "cargo install",
        "binstall_failed_fallback" => "downloader failed, falling back to cargo install",
        "summary_prebuilt" => "Prebuilt",
        "summary_compiled" => "Compiled",

        // Timing
        "total_time_seconds" => "in {}s",
        "total_time_millis" => "in {}ms",

        // Dry-run
        "dry_run_label" => "cargo",
        "dry_run_fallback_label" => "(fallback)",
        "dry_run_summary" => "no packages will be modified",

        // Completion install
        "completion_installed_path" => "completion to {}",
        "completion_path_exists" => "{} already exists",
        "completion_overwrite_prompt" => "Overwrite {}?",
        "completion_install_unsupported" => {
            "--install currently only supports fish; redirect stdout for other shells"
        }
        "completion_install_no_home" => {
            "cannot resolve install path for {shell}: $HOME / $XDG_CONFIG_HOME unset"
        }
        "completion_install_prompt" => {
            "Select which completions to install (space to toggle, enter to confirm)"
        }
        "completion_target_top" => "cargo-fresh<TAB>  — top-level binary completion",
        "completion_target_cargo" => "cargo fresh<TAB>  — cargo subcommand completion",
        "completion_install_summary" => "{written} written, {skipped} skipped",
        "completion_no_targets" => "no completion targets selected",

        _ => "",
    }
}

/// 获取中文文本
///
/// 根据文本键返回对应的中文文本
///
/// # Arguments
///
/// * `key` - 文本键，用于查找对应的中文文本
///
/// # Returns
///
/// 返回对应的中文文本，如果找不到则返回空字符串
pub fn get_chinese_text(key: &str) -> &'static str {
    match key {
        // 主要功能文本
        "checking_packages" => "全局 cargo 包更新",
        "no_packages_found" => "未找到匹配的已安装包",
        "found_packages" => "{} 个已安装的包",
        "all_up_to_date" => "所有包均为最新",

        // 选择提示（dialoguer 正文，保留完整句子）
        "stable_updates" => "稳定版更新：",
        "prerelease_updates" => "预发布版更新：",
        "prerelease_warning" => "预发布",
        "update_question" => "更新这些包？",
        "include_prerelease_question" => "是否包含预发布版？",
        "select_packages" => "选择要更新的包（空格切换，回车确认）",
        "no_interactive_mode" => "非交互模式，跳过选择",

        // 更新流程
        "starting_update" => "选中的包",
        "updating_package" => "{}",
        "package_failed" => "{}",
        "package_error" => "{name}：{error}",
        "update_completed" => "更新完成",

        // 统计（摘要内使用）
        "success_count" => "{} 个成功",
        "fail_count" => "{} 个失败",
        "no_packages_selected" => "未选择任何包",
        "aborted_by_user" => "用户取消，已完成 {done}/{total}",

        // 提示
        "update_instructions" => "手动更新：cargo install --force <name>",
        "interactive_instructions" => "或去掉 --no-interactive 进入交互模式",

        // 版本标签
        "version_unchanged" => "版本未变",
        "unknown_version" => "未知",
        "version_info_unknown" => "版本未知",
        "package_has_update" => "{}",
        "current_version" => "当前",
        "latest_version" => "最新",
        "package_up_to_date" => "{}",
        "version" => "版本",

        // 摘要
        "update_summary" => "更新摘要",
        "successful_updates" => "成功：",
        "failed_updates" => "失败：",
        "update_failed" => "失败",
        "updates_detected" => "可用更新：",
        "unknown" => "未知",

        // Updater 进度
        "updating_package_progress" => "{}",
        "current_version_label" => "当前",
        "executing_command" => "cargo",
        "retry_attempt" => "第 {attempt} 次重试 {name}",
        "error_message" => "stderr：",
        "package_update_success" => "{} 安装命令成功",
        "package_updated_version" => "{name} {old} -> {new}",
        "package_version_unchanged" => "{} 版本未变",
        "package_update_verification_failed" => "{} 命令成功但版本未验证",
        "package_update_failed" => "{name} 失败（退出码：{code}）",
        "error_details" => "详情：",
        "waiting_retry" => "等待重试",

        // Package 模块
        "cargo_install_list_failed" => "执行 cargo install --list 失败",
        "checking_package" => "检查",
        "unable_to_get_latest_version" => "无最新版本信息",
        "check_failed" => "检查失败",

        // Binstall
        "using_binstall" => "使用下载器",
        "using_install_fallback" => "cargo install",
        "binstall_failed_fallback" => "下载器失败，回退到 cargo install",
        "summary_prebuilt" => "预编译",
        "summary_compiled" => "源码编译",

        // 时间
        "total_time_seconds" => "耗时 {}s",
        "total_time_millis" => "耗时 {}ms",

        // Dry-run
        "dry_run_label" => "cargo",
        "dry_run_fallback_label" => "（回退）",
        "dry_run_summary" => "不会实际修改任何包",

        // 补全安装
        "completion_installed_path" => "补全脚本已写入 {}",
        "completion_path_exists" => "{} 已存在",
        "completion_overwrite_prompt" => "覆盖 {}？",
        "completion_install_unsupported" => {
            "--install 目前仅支持 fish，其它 shell 请通过重定向 stdout 安装"
        }
        "completion_install_no_home" => {
            "无法解析 {shell} 的安装路径：$HOME / $XDG_CONFIG_HOME 未设置"
        }
        "completion_install_prompt" => "选择要安装的补全（空格切换，回车确认）",
        "completion_target_top" => "cargo-fresh<TAB>  —— 顶层二进制补全",
        "completion_target_cargo" => "cargo fresh<TAB>  —— cargo 子命令补全",
        "completion_install_summary" => "写入 {written}，跳过 {skipped}",
        "completion_no_targets" => "未选择任何补全目标",

        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_english_texts() {
        assert_eq!(
            get_english_text("checking_packages"),
            "for updates to globally installed packages"
        );
        assert_eq!(
            get_english_text("all_up_to_date"),
            "all packages are up to date"
        );
        assert_eq!(get_english_text("nonexistent_key"), "");
    }

    #[test]
    fn test_chinese_texts() {
        assert_eq!(get_chinese_text("checking_packages"), "全局 cargo 包更新");
        assert_eq!(get_chinese_text("all_up_to_date"), "所有包均为最新");
        assert_eq!(get_chinese_text("nonexistent_key"), "");
    }

    #[test]
    fn test_text_consistency() {
        // 确保所有英文键在中文中也有对应的翻译
        let english_keys = [
            "checking_packages",
            "no_packages_found",
            "found_packages",
            "all_up_to_date",
            "stable_updates",
            "prerelease_updates",
            "prerelease_warning",
            "update_question",
            "include_prerelease_question",
            "select_packages",
            "starting_update",
            "updating_package",
            "package_failed",
            "package_error",
            "update_completed",
            "success_count",
            "fail_count",
            "no_packages_selected",
            "aborted_by_user",
            "update_instructions",
            "interactive_instructions",
            "version_unchanged",
            "unknown_version",
            "version_info_unknown",
            "package_has_update",
            "current_version",
            "latest_version",
            "package_up_to_date",
            "version",
            "update_summary",
            "summary_prebuilt",
            "summary_compiled",
            "successful_updates",
            "failed_updates",
            "update_failed",
            "updates_detected",
            "unknown",
            "updating_package_progress",
            "current_version_label",
            "executing_command",
            "retry_attempt",
            "error_message",
            "package_update_success",
            "package_updated_version",
            "package_version_unchanged",
            "package_update_verification_failed",
            "package_update_failed",
            "error_details",
            "waiting_retry",
            "unknown_version",
            "cargo_install_list_failed",
            "checking_package",
            "unable_to_get_latest_version",
            "check_failed",
            "completion_installed_path",
            "completion_path_exists",
            "completion_overwrite_prompt",
            "completion_install_unsupported",
            "completion_install_no_home",
            "completion_install_prompt",
            "completion_target_top",
            "completion_target_cargo",
            "completion_install_summary",
            "completion_no_targets",
        ];

        for key in &english_keys {
            let english = get_english_text(key);
            let chinese = get_chinese_text(key);

            // 确保键存在时，两种语言都有对应的文本
            if !english.is_empty() {
                assert!(
                    !chinese.is_empty(),
                    "Missing Chinese translation for key: {}",
                    key
                );
            }
        }
    }
}
