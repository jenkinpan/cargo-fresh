use semver::Version;

// 常量定义
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

    /// 判断是否有可用更新。
    ///
    /// 优先使用 semver 比较，仅当 `latest > current` 时才视为有更新——
    /// 这样可以避免被 yank 回滚（current > latest）触发误报。
    /// 任一版本字符串无法解析时 fallback 到字符串不等。
    pub fn has_update(&self) -> bool {
        match (&self.current_version, &self.latest_version) {
            (Some(current), Some(latest)) => {
                match (Version::parse(current), Version::parse(latest)) {
                    (Ok(c), Ok(l)) => l > c,
                    _ => current != latest,
                }
            }
            _ => false,
        }
    }

    /// 判断当前 `latest_version` 是否为预发布。
    ///
    /// 使用 `Version.pre.is_empty()` 进行 semver 标准判断，
    /// 避免旧版 `contains("rc")` 把含 "rc" 字面量的稳定版误报为预发布。
    /// 解析失败时返回 false（保守对待）。
    pub fn is_prerelease(&self) -> bool {
        self.latest_version
            .as_ref()
            .and_then(|v| Version::parse(v).ok())
            .map(|v| !v.pre.is_empty())
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
