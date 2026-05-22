//! 检查阶段的 binstall 预检。
//!
//! 对每个有更新的 crates.io 源包跑一次 `cargo binstall --dry-run`,提前判断
//! 这次升级会拿到预编译产物(快)还是退化成从源码构建(慢)。仅在 CLI
//! `--check-binstall` 时启用。
//!
//! 这针对的是一个真实痛点:某个 crate 刚发布新版,crates.io 已经有了,但
//! GitHub release 的预编译二进制还没传完——这段窗口期里 `cargo binstall`
//! 会闷头从源码编译十几分钟。预检在检查阶段就把这种包标成 `source build`,
//! 用户可以选择等一会儿再升,而不是启动后才发现卡住。

use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::Semaphore;

use crate::locale::detection::detect_language;
use crate::models::{BinstallKind, PackageInfo};

/// 同时运行的 dry-run 探针上限。每个探针都 spawn 一个 cargo 子进程并联网
/// (还含签名校验),比纯 index 请求重得多,所以并发数压得比版本检查保守。
const MAX_CONCURRENT_PROBES: usize = 8;

/// 纯函数:解析 `cargo binstall --dry-run` 的输出文本,判别预编译 / 源码构建。
///
/// 判别依据(实测 cargo-binstall 1.19,stdout/stderr 合并后传入):
/// - `has been downloaded from ...` —— 拿到预编译产物(GitHub release 与
///   QuickInstall 第三方源都打这行),归 `Prebuilt`。
/// - `will be installed from source` —— 找不到预编译产物,会退化成
///   `cargo install` 从源码构建,归 `SourceBuild`。
/// - 两者都没有(binstall 报错、crate 不存在、输出格式变化)—— `Unknown`。
///
/// 先判 source-build 再判 prebuilt:两条特征串不会同时出现,顺序只是为了
/// 让意图清晰——慢路径是我们最想可靠标出来的那个。
pub fn parse_dry_run(output: &str) -> BinstallKind {
    if output.contains("will be installed from source") {
        BinstallKind::SourceBuild
    } else if output.contains("has been downloaded from") {
        BinstallKind::Prebuilt
    } else {
        BinstallKind::Unknown
    }
}

/// 对单个 `name@version` 跑 `cargo binstall --dry-run --no-confirm`,返回判别
/// 结果。任何失败(spawn 失败、binstall 报错)都收敛成 `Unknown`——预检是
/// best-effort,绝不让它把主流程带崩。
pub async fn probe(package_name: &str, version: &str) -> BinstallKind {
    let spec = format!("{package_name}@{version}");
    match Command::new("cargo")
        .args(["binstall", "--dry-run", "--no-confirm", &spec])
        .output()
        .await
    {
        Ok(out) => {
            // binstall 的 INFO/WARN 走 stderr;把 stdout 也拼进来一起解析,
            // 防止它哪天换日志通道导致预检全部退化成 Unknown。
            let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stdout));
            parse_dry_run(&text)
        }
        Err(_) => BinstallKind::Unknown,
    }
}

/// 对所有"有更新的 crates.io 源包"并发跑 binstall 预检,把结论写回各自的
/// `PackageInfo.binstall_kind`。
///
/// 由 `main` 在 `--check-binstall` 且 binstall 已安装时调用,位置在
/// `check_package_updates` 之后——那时 `latest_version` / `has_update` 才就绪。
/// 探测前先打一行 `Checking` 状态:dry-run 每包约 10s,没有这行用户会以为卡住。
pub async fn annotate_updates(packages: &mut [PackageInfo]) {
    let targets: Vec<(usize, String, String)> = packages
        .iter()
        .enumerate()
        .filter(|(_, p)| p.source.is_crates() && p.has_update())
        .filter_map(|(i, p)| p.latest_version.clone().map(|v| (i, p.name.clone(), v)))
        .collect();

    if targets.is_empty() {
        return;
    }
    crate::display::status(
        "Checking",
        &detect_language()
            .get_text("checking_binstall")
            .replace("{}", &targets.len().to_string()),
    );

    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_PROBES));
    let mut handles = Vec::new();
    for (index, name, version) in targets {
        let sem = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok();
            (index, probe(&name, &version).await)
        }));
    }
    for handle in handles {
        if let Ok((index, kind)) = handle.await {
            packages[index].binstall_kind = Some(kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prebuilt_from_github_release() {
        // 实测输出:cargo-deny@0.19.7 有 GitHub release 预编译二进制
        let out = " INFO resolve: Resolving package: 'cargo-deny@=0.19.7'\n\
             WARN The package cargo-deny v0.19.7 (aarch64-apple-darwin) has been downloaded from github.com\n\
             INFO This will install the following binaries:\n";
        assert_eq!(parse_dry_run(out), BinstallKind::Prebuilt);
    }

    #[test]
    fn prebuilt_from_quickinstall() {
        // 实测输出:cargo-asm 走 QuickInstall 第三方预编译源——同样是 Prebuilt
        let out = " WARN The package cargo-asm v0.1.16 (aarch64-apple-darwin) has been \
             downloaded from third-party source QuickInstall\n\
             INFO This will install the following binaries:\n";
        assert_eq!(parse_dry_run(out), BinstallKind::Prebuilt);
    }

    #[test]
    fn source_build_when_no_prebuilt() {
        // 实测输出:cargo-count 没有预编译产物,binstall 会从源码构建——
        // 这正是用户那次卡 13 分钟的情况,必须可靠地标成 SourceBuild
        let out = " INFO resolve: Resolving package: 'cargo-count'\n\
             WARN The package cargo-count v0.2.4 will be installed from source (with cargo)\n\
             INFO Dry-run: running `cargo install cargo-count --version 0.2.4`\n";
        assert_eq!(parse_dry_run(out), BinstallKind::SourceBuild);
    }

    #[test]
    fn unknown_when_not_found_or_garbage() {
        let not_found = " ERROR Fatal error:\n  × cargo-no-such-crate is not found\n";
        assert_eq!(parse_dry_run(not_found), BinstallKind::Unknown);
        assert_eq!(parse_dry_run(""), BinstallKind::Unknown);
    }
}
