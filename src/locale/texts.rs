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
        // 主要功能文本
        "checking_packages" => "Checking for updates to globally installed Cargo packages...",
        "no_packages_found" => "No installed packages found",
        "found_packages" => "Found {} installed packages",
        "all_up_to_date" => "All packages are up to date!",

        // 更新相关文本
        "stable_updates" => "Stable version updates:",
        "prerelease_updates" => "Prerelease version updates:",
        "prerelease_warning" => "⚠️ Prerelease version",
        "update_question" => "Do you want to update these packages?",
        "include_prerelease_question" => "Include prerelease version updates?",
        "select_packages" => "Select packages to update (use space to select, enter to confirm)",
        "no_interactive_mode" => "Non-interactive mode detected. Skipping update selection.",

        // 更新过程文本
        "starting_update" => "Starting to update selected packages...",
        "updating_package" => "Updating {}...",
        "package_failed" => "❌ {} update failed",
        "package_error" => "❌ {name} update error: {error}",
        "update_completed" => "Update completed!",

        // 统计信息文本
        "success_count" => "Success: {} packages",
        "fail_count" => "Failed: {} packages",
        "no_packages_selected" => "No packages selected for update",

        // 帮助文本
        "update_instructions" => "To update packages, use: cargo install --force <package_name>",
        "interactive_instructions" => "Or remove --no-interactive flag for interactive updates",

        // 版本信息文本
        "version_unchanged" => "version unchanged",
        "unknown_version" => "unknown version",
        "version_info_unknown" => "version info unknown",
        "package_has_update" => "{} has updates available",
        "current_version" => "Current version:",
        "latest_version" => "Latest version:",
        "package_up_to_date" => "{} is up to date",
        "version" => "Version:",

        // 更新摘要文本
        "update_summary" => "📋 Update Summary",
        "successful_updates" => "✅ Successfully updated packages:",
        "failed_updates" => "❌ Failed to update packages:",
        "update_failed" => "update failed",
        "updates_detected" => "The following packages have updates available:",
        "unknown" => "unknown",

        // Updater模块文本
        "updating_package_progress" => "Updating {}...",
        "current_version_label" => "Current version:",
        "executing_command" => "Executing command:",
        "retry_attempt" => "Retry attempt {attempt} for {name}...",
        "error_message" => "Error message:",
        "package_update_success" => "✅ {} update command executed successfully",
        "package_updated_version" => "✅ {name} updated: {old} → {new}",
        "package_version_unchanged" => "⚠️ {} version unchanged, may already be the latest version",
        "package_update_verification_failed" => {
            "⚠️ {} update command successful but unable to verify new version"
        }
        "package_update_failed" => "❌ {name} update failed (exit code: {code})",
        "error_details" => "Error details:",
        "waiting_retry" => "Waiting before retry...",

        // Package模块文本
        "cargo_install_list_failed" => "Failed to execute cargo install --list",
        "checking_package" => "Checking",
        "unable_to_get_latest_version" => "unable to get latest version information",
        "check_failed" => "check failed",

        // Binstall相关文本
        "binstall_not_found" => "cargo binstall not found",
        "attempting_to_install_binstall" => "Attempting to install cargo binstall...",
        "installing_binstall" => "Installing cargo binstall...",
        "binstall_installed_successfully" => "cargo binstall installed successfully",
        "binstall_install_failed" => "Failed to install cargo binstall",
        "using_binstall" => "Using cargo binstall for faster installation",
        "using_install_fallback" => "Using cargo install as fallback",
        "binstall_failed_fallback" => "cargo binstall failed, falling back to cargo install",
        
        // 时间统计相关文本
        "total_time_seconds" => "Total time: {} seconds",
        "total_time_millis" => "Total time: {} milliseconds",

        // Dry-run 相关文本
        "dry_run_label" => "Would run:",
        "dry_run_fallback_label" => "(fallback would run:)",
        "dry_run_summary" => "🧪 Dry run — no packages were modified",

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
        "checking_packages" => "检查全局安装的 Cargo 包更新...",
        "no_packages_found" => "没有找到已安装的包",
        "found_packages" => "找到 {} 个已安装的包",
        "all_up_to_date" => "所有包都已是最新版本！",

        // 更新相关文本
        "stable_updates" => "稳定版本更新:",
        "prerelease_updates" => "预发布版本更新:",
        "prerelease_warning" => "⚠️ 预发布版本",
        "update_question" => "是否要更新这些包？",
        "include_prerelease_question" => "是否包含预发布版本更新？",
        "select_packages" => "选择要更新的包（使用空格选择，回车确认）",
        "no_interactive_mode" => "检测到非交互模式。跳过更新选择。",

        // 更新过程文本
        "starting_update" => "开始更新选中的包...",
        "updating_package" => "正在更新 {}...",
        "package_failed" => "❌ {} 更新失败",
        "package_error" => "❌ {name} 更新出错: {error}",
        "update_completed" => "更新完成！",

        // 统计信息文本
        "success_count" => "成功: {} 个包",
        "fail_count" => "失败: {} 个包",
        "no_packages_selected" => "未选择任何包进行更新",

        // 帮助文本
        "update_instructions" => "要更新包，请使用: cargo install --force <package_name>",
        "interactive_instructions" => "或者移除 --no-interactive 参数进行交互式更新",

        // 版本信息文本
        "version_unchanged" => "版本未改变",
        "unknown_version" => "未知版本",
        "version_info_unknown" => "版本信息未知",
        "package_has_update" => "{} 有更新可用",
        "current_version" => "当前版本:",
        "latest_version" => "最新版本:",
        "package_up_to_date" => "{} 已是最新版本",
        "version" => "版本:",

        // 更新摘要文本
        "update_summary" => "📋 更新摘要",
        "successful_updates" => "✅ 成功更新的包:",
        "failed_updates" => "❌ 更新失败的包:",
        "update_failed" => "更新失败",
        "updates_detected" => "检测到以下包有更新:",
        "unknown" => "未知",

        // Updater模块文本
        "updating_package_progress" => "正在更新 {}...",
        "current_version_label" => "当前版本:",
        "executing_command" => "执行命令:",
        "retry_attempt" => "重试第 {attempt} 次更新 {name}...",
        "error_message" => "错误信息:",
        "package_update_success" => "✅ {} 更新命令执行成功",
        "package_updated_version" => "✅ {name} 已更新: {old} → {new}",
        "package_version_unchanged" => "⚠️ {} 版本未改变，可能已经是最新版本",
        "package_update_verification_failed" => "⚠️ {} 更新命令成功但无法验证新版本",
        "package_update_failed" => "❌ {name} 更新失败 (退出码: {code})",
        "error_details" => "错误详情:",
        "waiting_retry" => "等待后重试...",

        // Package模块文本
        "cargo_install_list_failed" => "执行 cargo install --list 失败",
        "checking_package" => "检查",
        "unable_to_get_latest_version" => "无法获取最新版本信息",
        "check_failed" => "检查失败",

        // Binstall相关文本
        "binstall_not_found" => "未找到 cargo binstall",
        "attempting_to_install_binstall" => "正在尝试安装 cargo binstall...",
        "installing_binstall" => "正在安装 cargo binstall...",
        "binstall_installed_successfully" => "cargo binstall 安装成功",
        "binstall_install_failed" => "安装 cargo binstall 失败",
        "using_binstall" => "使用 cargo binstall 进行快速安装",
        "using_install_fallback" => "使用 cargo install 作为回退方案",
        "binstall_failed_fallback" => "cargo binstall 失败，回退到 cargo install",
        
        // 时间统计相关文本
        "total_time_seconds" => "总耗时: {} 秒",
        "total_time_millis" => "总耗时: {} 毫秒",

        // Dry-run 相关文本
        "dry_run_label" => "将执行:",
        "dry_run_fallback_label" => "(回退命令:)",
        "dry_run_summary" => "🧪 Dry run — 未实际修改任何包",

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
            "Checking for updates to globally installed Cargo packages..."
        );
        assert_eq!(
            get_english_text("all_up_to_date"),
            "All packages are up to date!"
        );
        assert_eq!(get_english_text("nonexistent_key"), "");
    }

    #[test]
    fn test_chinese_texts() {
        assert_eq!(
            get_chinese_text("checking_packages"),
            "检查全局安装的 Cargo 包更新..."
        );
        assert_eq!(get_chinese_text("all_up_to_date"), "所有包都已是最新版本！");
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
