// 常量定义
pub const PRERELEASE_KEYWORDS: &[&str] = &["alpha", "beta", "rc"];
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
pub const RETRY_DELAY_MS: u64 = 2000;
pub const VERSION_UPDATE_DELAY_MS: u64 = 1000;
pub const PROGRESS_TICK_MS: u64 = 100;
pub const PROGRESS_BAR_WIDTH: usize = 40;

#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub package_name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub success: bool,
}

impl PackageInfo {
    pub fn new(name: String, current_version: Option<String>) -> Self {
        Self {
            name,
            current_version,
            latest_version: None,
        }
    }

    pub fn has_update(&self) -> bool {
        matches!(
            (&self.current_version, &self.latest_version),
            (Some(current), Some(latest)) if current != latest
        )
    }

    pub fn is_prerelease(&self) -> bool {
        self.latest_version
            .as_ref()
            .map(|v| {
                PRERELEASE_KEYWORDS
                    .iter()
                    .any(|&keyword| v.contains(keyword))
            })
            .unwrap_or(false)
    }
}

impl UpdateResult {
    pub fn new(
        package_name: String,
        old_version: Option<String>,
        new_version: Option<String>,
        success: bool,
    ) -> Self {
        Self {
            package_name,
            old_version,
            new_version,
            success,
        }
    }
}
