/// Language detection functionality
///
/// 通过环境变量 (LANG / LC_ALL / LC_CTYPE) 自动检测系统语言偏好。
use super::language::Language;

/// 纯函数：根据 locale 字符串判定语言。
///
/// 拆出来是为了让单元测试不再 `env::set_var`——之前并发跑测试时
/// 谁也不能保证 set/restore 之间没有别的线程读到中间值，CI 偶发挂掉。
pub fn detect_from_locale(locale: &str) -> Language {
    if locale.starts_with("zh") || locale.contains("zh_CN") || locale.contains("zh_TW") {
        Language::Chinese
    } else {
        Language::English
    }
}

/// 检测系统语言环境
///
/// 优先级：LANG > LC_ALL > LC_CTYPE，无任何值时回退到英文。
pub fn detect_language() -> Language {
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_CTYPE"))
        .unwrap_or_else(|_| "en_US.UTF-8".to_string());
    detect_from_locale(&locale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_chinese_locales() {
        assert_eq!(detect_from_locale("zh_CN.UTF-8"), Language::Chinese);
        assert_eq!(detect_from_locale("zh_TW.UTF-8"), Language::Chinese);
        assert_eq!(detect_from_locale("zh"), Language::Chinese);
    }

    #[test]
    fn detects_english_locales() {
        assert_eq!(detect_from_locale("en_US.UTF-8"), Language::English);
        assert_eq!(detect_from_locale("en_GB.UTF-8"), Language::English);
    }

    #[test]
    fn unknown_locale_defaults_to_english() {
        assert_eq!(detect_from_locale(""), Language::English);
        assert_eq!(detect_from_locale("fr_FR.UTF-8"), Language::English);
        assert_eq!(detect_from_locale("ja_JP.UTF-8"), Language::English);
    }
}
