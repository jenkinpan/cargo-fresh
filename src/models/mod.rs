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

    /// JSON `skipped[].reason_code`——稳定可判别的枚举字符串。
    /// `skipped[]` 只收非 crates 源，`Crates` 分支不会被实际输出。
    pub fn skip_reason_code(&self) -> &'static str {
        match self {
            PackageSource::Crates => "crates_source",
            PackageSource::Git { .. } => "git_source",
            PackageSource::Path { .. } => "path_source",
            PackageSource::Unknown(_) => "unknown_source",
        }
    }
}

/// 一个包安装时使用的 Cargo 特性选项，从 `$CARGO_HOME/.crates2.json` 解析而来。
///
/// 只建模 features 三项；profile/target/rustc 刻意不保留（见 spec）。
/// `None`（在 `PackageInfo.install_opts` 上）表示没读到元数据，走默认行为。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstallOpts {
    pub no_default_features: bool,
    pub all_features: bool,
    pub features: Vec<String>,
}

impl InstallOpts {
    /// 全默认安装——可安全走 binstall，无需追加任何 cargo flag。
    pub fn is_default(&self) -> bool {
        !self.no_default_features && !self.all_features && self.features.is_empty()
    }
}

/// 版本检查失败的可判别分类。决定 JSON `version_check_errors[].kind`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckErrorKind {
    /// registry index 里没有这个包（4xx）——重试无意义，可能是改名 / 配错 registry。
    NotFound,
    /// 网络 / 超时 / 5xx / 解析失败——瞬时故障，重试 CI 作业可能恢复。
    Unavailable,
}

impl CheckErrorKind {
    /// JSON 里用 "not_found" / "unavailable" 短串表示。
    pub fn kind_str(&self) -> &'static str {
        match self {
            CheckErrorKind::NotFound => "not_found",
            CheckErrorKind::Unavailable => "unavailable",
        }
    }
}

/// 一个包版本检查失败的记录。`message` 是人读的文案，不保证稳定、不要据此分支。
#[derive(Debug, Clone)]
pub struct CheckError {
    pub kind: CheckErrorKind,
    pub message: String,
}

/// Downloader-side prebuilt-binary availability — what `cargo-fresh` would do
/// if asked to update this package right now. Ternary so we can distinguish
/// "downloader确实没找到" from "网络/服务端有问题没法判断".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrebuiltAvailability {
    /// At least one candidate URL returned 2xx — downloader will succeed.
    Prebuilt,
    /// All candidates returned 4xx (typically 404) — downloader will fall back
    /// to `cargo install` (compile from source).
    Source,
    /// Probe could not reach a verdict (all 5xx, all timed out, or network
    /// error). Don't blame downloader; treat as "try again later".
    Unknown,
}

impl PrebuiltAvailability {
    pub fn kind_str(self) -> &'static str {
        match self {
            PrebuiltAvailability::Prebuilt => "prebuilt",
            PrebuiltAvailability::Source => "source",
            PrebuiltAvailability::Unknown => "unknown",
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            PrebuiltAvailability::Prebuilt => "[prebuilt]",
            PrebuiltAvailability::Source => "[source]",
            PrebuiltAvailability::Unknown => "[probe failed]",
        }
    }
}

#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub source: PackageSource,
    pub install_opts: Option<InstallOpts>,
    pub check_error: Option<CheckError>,
    /// Downloader probe result, populated when `--check-prebuilt` runs.
    /// `None` = not probed (flag absent, or package isn't a crates.io update candidate).
    pub prebuilt: Option<PrebuiltAvailability>,
}

/// 这次更新走了哪条安装路径——给汇总分组用 (rustup 风格:
/// 末尾告诉用户哪些走的预编译, 哪些是源码编译, 不在过程中刷)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstallMethod {
    /// 自托管 downloader 路径——GitHub Release 预编译二进制
    Downloader,
    /// `cargo install` 子进程——本地编译 (慢)
    CargoInstall,
    /// 未知 / 未尝试 (失败或中止)
    #[default]
    Unknown,
}

impl InstallMethod {
    /// JSON `results[].install_method` 的取值——刻意复用 `PrebuiltAvailability`
    /// 的词汇表（`"prebuilt"` / `"source"`），这样脚本能用同一组枚举把
    /// `updates_available[].prebuilt`（预测）和 `results[].install_method`（实际）
    /// 直接对比。`Unknown`（失败/中止，没走到安装）映射成 `None` → JSON `null`，
    /// 语义是“不适用”，区别于探测失败的 `"unknown"`。
    pub fn json_str(self) -> Option<&'static str> {
        match self {
            InstallMethod::Downloader => Some("prebuilt"),
            InstallMethod::CargoInstall => Some("source"),
            InstallMethod::Unknown => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateResult {
    pub package_name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub success: bool,
    pub install_method: InstallMethod,
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
            install_opts: None,
            check_error: None,
            prebuilt: None,
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
            .is_some_and(|v| !v.pre.is_empty())
    }
}

/// JSON 输出 schema v2。`--format=json` 在 main 末尾把整个流程的快照
/// 写到 stdout 一行——脚本可以直接 `jq` 消费，无需解析 ANSI 文案。
///
/// 字段约定：
/// - `schema_version`：当前为 `2`；同一大版本内只做向后兼容的字段新增，
///   rename / remove 才 bump（0.12.0 由 1 → 2）
/// - `updates_available`：所有有更新候选的包（不论是否被 batch / 用户选中）
/// - `results`：实际跑过 cargo install 的包及其结果；非 batch / 无 dry-run 时为空数组
/// - `summary.duration_ms` 是整个执行的墙钟时间（含检查 + 更新）
#[derive(Debug, Clone, Serialize)]
pub struct JsonReport<'a> {
    pub schema_version: u32,
    pub format: &'static str,
    /// 产出这份报告的 cargo-fresh 版本（`env!("CARGO_PKG_VERSION")`）。
    /// 让归档/issue 里贴的 JSON 自描述，无需另问“你跑的哪个版本”。
    pub version: &'static str,
    pub include_prerelease: bool,
    pub dry_run: bool,
    pub registry_url: Option<&'a str>,
    pub updates_available: Vec<JsonUpdateCandidate<'a>>,
    pub fresh: Vec<&'a str>,
    pub skipped: Vec<JsonSkipped<'a>>,
    pub version_check_errors: Vec<JsonCheckError<'a>>,
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
    /// Downloader 预编译可用性:`"prebuilt"` / `"source"` / `"unknown"`,
    /// 未跑 `--check-prebuilt` 时为 `null`。0.12 起取代旧字段 `binstall`。
    pub prebuilt: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSkipped<'a> {
    pub name: &'a str,
    pub source: &'static str,
    pub reason_code: &'static str,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonCheckError<'a> {
    pub name: &'a str,
    pub kind: &'static str,
    pub error: &'a str,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonResult<'a> {
    pub name: &'a str,
    pub old_version: Option<&'a str>,
    pub new_version: Option<&'a str>,
    pub success: bool,
    /// 实际走的安装路径：`"prebuilt"`（downloader）/ `"source"`（cargo install）/
    /// `null`（失败/中止，没走到安装）。与 `updates_available[].prebuilt` 共用词汇表，
    /// 便于脚本对比“预测 vs 实际”。
    pub install_method: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSummary {
    pub checked: usize,
    pub available: usize,
    pub selected: usize,
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    pub check_errors: usize,
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
            install_method: InstallMethod::Unknown,
        }
    }

    /// 链式 setter——更新成功的路径调用方在拿到 UpdateResult 后挂上方法标记。
    pub fn with_install_method(mut self, method: InstallMethod) -> Self {
        self.install_method = method;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_opts_default_is_default() {
        let o = InstallOpts::default();
        assert!(o.is_default());
    }

    #[test]
    fn install_opts_with_features_is_not_default() {
        let o = InstallOpts {
            no_default_features: false,
            all_features: false,
            features: vec!["pcre2".to_string()],
        };
        assert!(!o.is_default());
    }

    #[test]
    fn install_opts_no_default_features_is_not_default() {
        let o = InstallOpts {
            no_default_features: true,
            all_features: false,
            features: vec![],
        };
        assert!(!o.is_default());
    }

    #[test]
    fn install_opts_all_features_is_not_default() {
        let o = InstallOpts {
            no_default_features: false,
            all_features: true,
            features: vec![],
        };
        assert!(!o.is_default());
    }

    #[test]
    fn package_info_install_opts_defaults_none() {
        let p = PackageInfo::new("ripgrep".to_string(), Some("14.0.0".to_string()));
        assert!(p.install_opts.is_none());
    }

    #[test]
    fn skip_reason_code_maps_each_source() {
        assert_eq!(
            PackageSource::Path { dir: "/x".into() }.skip_reason_code(),
            "path_source"
        );
        assert_eq!(
            PackageSource::Git { url: "u".into(), rev: None }.skip_reason_code(),
            "git_source"
        );
        assert_eq!(
            PackageSource::Unknown("weird".into()).skip_reason_code(),
            "unknown_source"
        );
    }

    #[test]
    fn check_error_kind_str_maps_both_variants() {
        assert_eq!(CheckErrorKind::NotFound.kind_str(), "not_found");
        assert_eq!(CheckErrorKind::Unavailable.kind_str(), "unavailable");
    }

    #[test]
    fn prebuilt_availability_kind_str() {
        assert_eq!(PrebuiltAvailability::Prebuilt.kind_str(), "prebuilt");
        assert_eq!(PrebuiltAvailability::Source.kind_str(), "source");
        assert_eq!(PrebuiltAvailability::Unknown.kind_str(), "unknown");
    }

    #[test]
    fn prebuilt_availability_marker() {
        assert_eq!(PrebuiltAvailability::Prebuilt.marker(), "[prebuilt]");
        assert_eq!(PrebuiltAvailability::Source.marker(), "[source]");
        assert_eq!(PrebuiltAvailability::Unknown.marker(), "[probe failed]");
    }
}
