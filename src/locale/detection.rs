/// Language detection functionality
///
/// This module handles automatic language detection based on system
/// environment variables (LANG, LC_ALL, LC_CTYPE).
use super::language::Language;

/// 检测系统语言环境
///
/// 通过检查环境变量来确定用户的语言偏好
/// 支持的环境变量（按优先级）：
/// - LANG: 主要语言设置
/// - LC_ALL: 全局语言设置
/// - LC_CTYPE: 字符类型设置
///
/// # Returns
///
/// 返回检测到的语言类型，默认为英文
pub fn detect_language() -> Language {
    // 检查环境变量来确定语言
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_CTYPE"))
        .unwrap_or_else(|_| "en_US.UTF-8".to_string());

    // 检查是否是中文环境
    if locale.starts_with("zh") || locale.contains("zh_CN") || locale.contains("zh_TW") {
        Language::Chinese
    } else {
        Language::English
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chinese_language() {
        std::env::set_var("LANG", "zh_CN.UTF-8");
        assert_eq!(detect_language(), Language::Chinese);

        std::env::set_var("LANG", "zh_TW.UTF-8");
        assert_eq!(detect_language(), Language::Chinese);

        std::env::set_var("LANG", "zh");
        assert_eq!(detect_language(), Language::Chinese);
    }

    #[test]
    fn test_detect_english_language() {
        std::env::set_var("LANG", "en_US.UTF-8");
        assert_eq!(detect_language(), Language::English);

        std::env::set_var("LANG", "en_GB.UTF-8");
        assert_eq!(detect_language(), Language::English);
    }

    #[test]
    fn test_detect_default_language() {
        std::env::remove_var("LANG");
        std::env::remove_var("LC_ALL");
        std::env::remove_var("LC_CTYPE");
        assert_eq!(detect_language(), Language::English);
    }
}
