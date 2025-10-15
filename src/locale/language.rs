/// Language type definitions and implementations
///
/// This module defines the supported languages and provides
/// the main Language enum with its methods.
/// 支持的语言类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    /// 获取语言对应的文本
    ///
    /// 根据当前语言类型返回对应的本地化文本
    ///
    /// # Arguments
    ///
    /// * `key` - 文本键，用于查找对应的本地化文本
    ///
    /// # Returns
    ///
    /// 返回对应的本地化文本，如果找不到则返回空字符串
    pub fn get_text(&self, key: &str) -> &'static str {
        match self {
            Language::English => crate::locale::texts::get_english_text(key),
            Language::Chinese => crate::locale::texts::get_chinese_text(key),
        }
    }

    /// 获取语言名称
    ///
    /// # Returns
    ///
    /// 返回当前语言的英文名称
    pub fn name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Chinese => "Chinese",
        }
    }

    /// 获取语言代码
    ///
    /// # Returns
    ///
    /// 返回当前语言的 ISO 语言代码
    #[allow(dead_code)]
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
        }
    }

    /// 检查是否为中文
    ///
    /// # Returns
    ///
    /// 如果是中文返回 true，否则返回 false
    #[allow(dead_code)]
    pub fn is_chinese(&self) -> bool {
        matches!(self, Language::Chinese)
    }

    /// 检查是否为英文
    ///
    /// # Returns
    ///
    /// 如果是英文返回 true，否则返回 false
    #[allow(dead_code)]
    pub fn is_english(&self) -> bool {
        matches!(self, Language::English)
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_names() {
        assert_eq!(Language::English.name(), "English");
        assert_eq!(Language::Chinese.name(), "Chinese");
    }

    #[test]
    fn test_language_codes() {
        assert_eq!(Language::English.code(), "en");
        assert_eq!(Language::Chinese.code(), "zh");
    }

    #[test]
    fn test_language_checks() {
        assert!(Language::English.is_english());
        assert!(!Language::English.is_chinese());

        assert!(Language::Chinese.is_chinese());
        assert!(!Language::Chinese.is_english());
    }

    #[test]
    fn test_language_display() {
        assert_eq!(format!("{}", Language::English), "English");
        assert_eq!(format!("{}", Language::Chinese), "Chinese");
    }
}
