use semver::Version;
use serde::Serialize;

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
    /// `cargo install --list` 中无法识别的来源前缀。
    ///
    /// 旧版会把这些悄悄归到 `Crates`，导致 cargo-fresh 试图去 sparse index
    /// 查一个其实不是 crates.io 来源的包，把人误导到"版本检查失败"。
    /// 现在显式建模——`check_package_updates` 跳过，`updater::build_args`
    /// 直接拒绝，UI 用 `[unknown source]` 标记，让用户看到这个包被有意忽略。
    Unknown(String),
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
            PackageSource::Unknown(_) => "[unknown source]",
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

/// JSON 输出 schema v1。`--format=json` 在 main 末尾把整个流程的快照
/// 写到 stdout 一行——脚本可以直接 `jq` 消费，无需解析 ANSI 文案。
///
/// 字段约定：
/// - `schema_version`：单调递增，1.x 内只做向后兼容的字段新增
/// - `updates_available`：所有有更新候选的包（不论是否被 batch / 用户选中）
/// - `results`：实际跑过 cargo install 的包及其结果；非 batch / 无 dry-run 时为空数组
/// - `summary.duration_ms` 是整个执行的墙钟时间（含检查 + 更新）
#[derive(Debug, Clone, Serialize)]
pub struct JsonReport<'a> {
    pub schema_version: u32,
    pub format: &'static str,
    pub include_prerelease: bool,
    pub dry_run: bool,
    pub registry_url: Option<&'a str>,
    pub updates_available: Vec<JsonUpdateCandidate<'a>>,
    pub fresh: Vec<&'a str>,
    pub skipped: Vec<JsonSkipped<'a>>,
    pub results: Vec<JsonResult<'a>>,
    pub summary: JsonSummary,
    pub aborted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonUpdateCandidate<'a> {
    pub name: &'a str,
    pub current: Option<&'a str>,
    pub latest: &'a str,
    pub source: &'static str,
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSkipped<'a> {
    pub name: &'a str,
    pub source: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonResult<'a> {
    pub name: &'a str,
    pub old_version: Option<&'a str>,
    pub new_version: Option<&'a str>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSummary {
    pub checked: usize,
    pub available: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u128,
}

/// PackageSource 在 JSON 里用 "crates" / "git" / "path" 短串表示。
impl PackageSource {
    pub fn kind_str(&self) -> &'static str {
        match self {
            PackageSource::Crates => "crates",
            PackageSource::Git { .. } => "git",
            PackageSource::Path { .. } => "path",
            PackageSource::Unknown(_) => "unknown",
        }
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
