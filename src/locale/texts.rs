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
        "update_question" => "Do you want to update these packages? [Y/n]:",
        "include_prerelease_question" => "Include prerelease version updates? [y/N]:",
        "select_packages" => "Select packages to update (use space to select, enter to confirm)",

        // 更新过程文本
        "starting_update" => "Starting to update selected packages...",
        "updating_package" => "Updating {}...",
        "package_updated" => "✅ {} updated: {} → {}",
        "package_failed" => "❌ {} update failed",
        "package_error" => "❌ {} update error: {}",
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
        "update_question" => "是否要更新这些包？ [Y/n]:",
        "include_prerelease_question" => "是否包含预发布版本更新？ [y/N]:",
        "select_packages" => "选择要更新的包（使用空格选择，回车确认）",

        // 更新过程文本
        "starting_update" => "开始更新选中的包...",
        "updating_package" => "正在更新 {}...",
        "package_updated" => "✅ {} 已更新: {} → {}",
        "package_failed" => "❌ {} 更新失败",
        "package_error" => "❌ {} 更新出错: {}",
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
            "package_updated",
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
