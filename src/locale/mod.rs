/// Locale module for internationalization support
///
/// This module provides language detection and text localization functionality.
/// It supports both English and Chinese languages with automatic detection
/// based on system environment variables.
pub mod detection;
pub mod language;
pub mod texts;

pub use detection::detect_language;
pub use language::Language;
// Text functions are available through the Language enum's get_text method

// Re-export the main functionality for convenience
// pub use language::Language as LocaleLanguage;
