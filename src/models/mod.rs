use semver::Version;

// 常量定义
pub const MAX_RETRY_ATTEMPTS: u32 = 3;
pub const RETRY_DELAY_MS: u64 = 2000;
pub const VERSION_UPDATE_DELAY_MS: u64 = 1000;
pub const PROGRESS_TICK_MS: u64 = 100;

/// 包的安装来源。
///
/// `cargo install --list` 输出会在 `name vVERSION` 之后附带括号标识来源，
/// cargo-fresh 用它决定升级时该用 `cargo install`（crates.io）、
/// `cargo install --git URL`（git）还是 `cargo install --path DIR`（本地）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    /// crates.io 注册表（默认或自定义 registry）
    Crates,
    /// git 仓库；rev 是固定的 commit / branch / tag
    Git {
        url: String,
        rev: Option<String>,
    },
    /// 本地路径
    Path {
        dir: String,
    },
}

impl PackageSource {
    /// 是否为 crates.io 来源——决定能否做版本检查、能否走 binstall
    pub fn is_crates(&self) -> bool {
        matches!(self, PackageSource::Crates)
    }

    /// 给 UI 显示的短标记，例如 `[git]`、`[path]`，crates 不加标记返回空字串
    pub fn marker(&self) -> &'static str {
        match self {
            PackageSource::Crates => "",
            PackageSource::Git { .. } => "[git]",
            PackageSource::Path { .. } => "[path]",
        }
    }
}

#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub source: PackageSource,
}

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub package_name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub success: bool,
}

impl PackageInfo {
    /// 便利构造器：默认 crates.io 源，主要供测试使用
    #[allow(dead_code)]
    pub fn new(name: String, current_version: Option<String>) -> Self {
        Self::with_source(name, current_version, PackageSource::Crates)
    }

    pub fn with_source(
        name: String,
        current_version: Option<String>,
        source: PackageSource,
    ) -> Self {
        Self {
            name,
            current_version,
            latest_version: None,
            source,
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
