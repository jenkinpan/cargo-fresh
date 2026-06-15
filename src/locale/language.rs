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

    /// 用命名占位符填充本地化模板
    ///
    /// 模板中形如 `{name}` 的占位符会被 `args` 中对应键的值替换。
    /// 相比反复链式 `replace("{}", x)`（会一次性替换所有 `{}`），
    /// 命名占位符保证每个变量只替换到自己的位置。
    ///
    /// 已知限制：实现按 `args` 顺序串行替换，所以一个变量的值若包含后续变量的
    /// 占位符字面量（如 value 里写 `{error}`），会被再次展开。本项目中所有
    /// 实际取值都是包名/版本号/错误信息，不含此模式，因此不做防御。
    pub fn format_text(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut s = self.get_text(key).to_string();
        for (name, value) in args {
            s = s.replace(&format!("{{{name}}}"), value);
        }
        s
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

    #[test]
    fn test_format_text_single_named_placeholder() {
        // 单个命名占位符替换
        let out = Language::English.format_text("package_error", &[
            ("name", "ripgrep"),
            ("error", "boom"),
        ]);
        assert_eq!(out, "ripgrep: boom");
    }

    #[test]
    fn test_format_text_multi_named_placeholders_no_collision() {
        // 这是修复 i18n bug 的关键回归测试：
        // 旧代码用链式 .replace("{}", x) 会把所有 {} 都替换成第一个值。
        // 命名占位符必须保证 {name}/{old}/{new} 各自只替换到自己的位置。
        let out = Language::English.format_text("package_updated_version", &[
            ("name", "ripgrep"),
            ("old", "13.0.0"),
            ("new", "14.1.0"),
        ]);
        assert_eq!(out, "ripgrep 13.0.0 -> 14.1.0");
    }

    #[test]
    fn test_format_text_missing_arg_leaves_placeholder() {
        // 缺失参数时占位符原样保留，便于及时发现遗漏的 key
        let out = Language::English.format_text("package_update_failed", &[
            ("name", "tokei"),
            // 故意不传 code
        ]);
        assert_eq!(out, "tokei failed (exit code: {code})");
    }

    #[test]
    fn test_format_text_works_for_chinese_template() {
        // 中文模板同样使用命名占位符
        let out = Language::Chinese.format_text("retry_attempt", &[
            ("attempt", "2"),
            ("name", "cargo-fresh"),
        ]);
        assert_eq!(out, "第 2 次重试 cargo-fresh");
    }
}
