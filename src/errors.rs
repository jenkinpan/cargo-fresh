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

/// 把 anyhow 错误链嗅探成可执行提示。返回 `None` 时调用方按原样打印错误。
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
pub fn hint_for(err: &anyhow::Error) -> Option<&'static str> {
    for cause in err.chain() {
        if let Some(cf) = cause.downcast_ref::<CargoFreshError>() {
            return Some(match cf {
                CargoFreshError::CargoListFailed { .. } => {
                    "Is `cargo` on your PATH? Try `cargo --version` to verify the toolchain."
                }
            });
        }
        if let Some(re) = cause.downcast_ref::<reqwest::Error>() {
            if re.is_connect() || re.is_timeout() {
                return Some(
                    "Network connect/timeout. Check connectivity to index.crates.io, \
                     or set HTTPS_PROXY if behind a proxy.",
                );
            }
        }
        if cause.downcast_ref::<globset::Error>().is_some() {
            return Some(
                "Invalid glob in `--filter` / `--exclude`. Patterns use glob syntax \
                 (`*`, `?`, `[abc]`); quote the pattern in your shell and close any `[` bracket.",
            );
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
        assert!(hint_for(&err).unwrap().contains("cargo --version"));
    }

    #[test]
    fn hint_for_wrapped_cargo_fresh_error() {
        // anyhow 的 context 链：底层是 CargoFreshError，外层 wrap 了一层
        let err = anyhow::Error::from(CargoFreshError::CargoListFailed {
            source: anyhow::anyhow!("exit code 101"),
        })
        .context("while listing installed packages");
        assert!(hint_for(&err).unwrap().contains("cargo --version"));
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
        assert!(hint_for(&err).unwrap().contains("--filter"));
    }
}
