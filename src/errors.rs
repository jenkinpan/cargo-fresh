//! cargo-fresh 的可执行错误类型与提示映射。
//!
//! 大部分内部路径继续返回 `anyhow::Result`——错误链很短，没必要为每个 bail
//! 都写一个枚举变体。这个模块的核心价值是 [`hint_for`]：拿到失败时的
//! `anyhow::Error`，沿错误链向下嗅探到 `CargoFreshError` 或底层
//! `reqwest::Error`，反推一条对用户**可执行**的提示串（"检查 HTTPS_PROXY"、
//! "用 cargo --version 验证 toolchain" 之类），由 `main` 在退出前打到 stderr。
//!
//! 取舍：与其逼着所有 `anyhow::bail!` 改成枚举，不如保留 anyhow 的灵活
//! 上下文，仅在我们能识别且能给出**具体**建议的路径上下沉到这里。
//! 没有匹配时 `hint_for` 返回 `None`，由调用方退回默认错误信息。

use thiserror::Error;

/// 用户运行 cargo-fresh 时能识别出的几类典型失败。
///
/// 目前只覆盖会真正冒泡到 `main` 的路径——`fetch_latest_versions` 内部把
/// 网络错误全部吞成空候选，所以 registry 相关的失败不会以错误形式出现。
/// 网络层提示由 `hint_for` 中对 `reqwest::Error` 的直接匹配兜底，未来若
/// 决定把 registry 错误外抛，可在此处增补变体。
#[derive(Debug, Error)]
pub enum CargoFreshError {
    /// `cargo install --list` 子进程跑失败：cargo 不在 PATH、cargo home 损坏等。
    #[error("`cargo install --list` failed: {source}")]
    CargoListFailed {
        #[source]
        source: anyhow::Error,
    },
}

/// 一条可执行提示。`hint_for` 只返回**哪一条**提示（locale key），具体文案由
/// `main` 通过 `Language::get_text(hint.locale_key())` 按用户语言渲染——和 cargo-fresh
/// 其它所有用户可见字符串走同一条 i18n 路径（`src/locale/texts.rs`），不在这里硬编码英文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hint {
    /// `cargo install --list` 子进程失败：检查 cargo 是否在 PATH。
    CargoListFailed,
    /// reqwest 连接/超时：检查网络连通性 / 代理。
    NetworkConnectTimeout,
    /// `--filter` / `--exclude` 传了非法 glob。
    InvalidGlob,
}

impl Hint {
    /// 对应 `src/locale/texts.rs` 里的本地化键，喂给 `Language::get_text`。
    pub fn locale_key(self) -> &'static str {
        match self {
            Hint::CargoListFailed => "hint_cargo_list_failed",
            Hint::NetworkConnectTimeout => "hint_network_connect_timeout",
            Hint::InvalidGlob => "hint_invalid_glob",
        }
    }
}

/// 把 anyhow 错误链嗅探成一条 [`Hint`]。返回 `None` 时调用方按原样打印错误。
///
/// 嗅探规则：
/// 1. 先看错误链里是否有 `CargoFreshError` —— 我们自己显式建模的几个路径
/// 2. 再看链里是否有 `reqwest::Error` 且 `is_connect()` / `is_timeout()` —— 网络层判定
/// 3. 再看链里是否有 `globset::Error` —— `--filter` / `--exclude` 传了非法 glob
/// 4. 都不匹配返回 None
///
/// 注意：每包安装失败（权限被拒、GitHub 限流等）不会走到这里——它们被
/// `run_one_update` 收成 `Failed` 结果、单独打 `Failed` 行，从不以顶层
/// `anyhow::Error` 形式冒泡。所以这里只覆盖 `run()` 里 `?` 直接外抛的几条
/// 启动期路径（列包、过滤模式编译、网络）。
pub fn hint_for(err: &anyhow::Error) -> Option<Hint> {
    for cause in err.chain() {
        if cause.downcast_ref::<CargoFreshError>().is_some() {
            // 目前 CargoFreshError 只有 CargoListFailed 一个变体。
            return Some(Hint::CargoListFailed);
        }
        if let Some(re) = cause.downcast_ref::<reqwest::Error>() {
            if re.is_connect() || re.is_timeout() {
                return Some(Hint::NetworkConnectTimeout);
            }
        }
        if cause.downcast_ref::<globset::Error>().is_some() {
            return Some(Hint::InvalidGlob);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_for_cargo_list_failed() {
        let err: anyhow::Error = CargoFreshError::CargoListFailed {
            source: anyhow::anyhow!("exit code 101"),
        }
        .into();
        assert_eq!(hint_for(&err), Some(Hint::CargoListFailed));
    }

    #[test]
    fn hint_for_wrapped_cargo_fresh_error() {
        // anyhow 的 context 链：底层是 CargoFreshError，外层 wrap 了一层
        let err = anyhow::Error::from(CargoFreshError::CargoListFailed {
            source: anyhow::anyhow!("exit code 101"),
        })
        .context("while listing installed packages");
        assert_eq!(hint_for(&err), Some(Hint::CargoListFailed));
    }

    #[test]
    fn hint_for_unrelated_error_returns_none() {
        let err = anyhow::anyhow!("something else entirely");
        assert!(hint_for(&err).is_none());
    }

    #[test]
    fn hint_for_invalid_glob() {
        // 走真实的 filter_packages 路径,确保 globset 错误真的能被嗅探到
        // （而不是手搓一个假的 globset::Error 类型）。
        let mut packages = Vec::new();
        let err = crate::package::filter_packages(&mut packages, "[unclosed")
            .expect_err("unclosed bracket should be an invalid glob");
        assert_eq!(hint_for(&err), Some(Hint::InvalidGlob));
    }

    #[test]
    fn every_hint_has_bilingual_text() {
        // 每个 Hint 变体的 locale_key 在英文和中文里都必须有非空文案——
        // 否则 main 会打出空 `Hint:` 行。
        use crate::locale::texts::{get_chinese_text, get_english_text};
        for hint in [
            Hint::CargoListFailed,
            Hint::NetworkConnectTimeout,
            Hint::InvalidGlob,
        ] {
            let key = hint.locale_key();
            assert!(!get_english_text(key).is_empty(), "missing EN for {key}");
            assert!(!get_chinese_text(key).is_empty(), "missing ZH for {key}");
        }
    }
}
