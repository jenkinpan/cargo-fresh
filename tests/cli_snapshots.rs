//! 锁住 status 行的 cargo-style 外观契约——verb 名、12 字右对齐、
//! 来源/prebuilt 尾标的拼装格式。CLAUDE.md 的 "Status verb dictionary"
//! 把这些列成了事实合约,README/issue templates 教用户读这些行,但代码里
//! 没有任何东西阻止"顺手把 `Fresh` 改成 `UpToDate`"或者"把宽度从 12
//! 改到 10"。snapshot 在 PR diff 上把这种漂移变成 `.snap` 文件改动,
//! reviewer 一眼能看见。
//!
//! 颜色 ANSI 用 insta 的 filter 剥掉——颜色契约已经被
//! `tests/cli.rs::no_color_env_strips_ansi_from_stderr` 等专门测试覆盖,
//! 这层只管 verb 名 + 对齐 + 行结构,分工干净 `.snap` 文件也可读。
//!
//! 范围控制:8 条核心 verb——CLAUDE.md verb 字典里使用频次最高 / 用户脚本
//! 最可能 grep 的几个 + `--check-prebuilt` 尾标(0.12.0 新加,UX 决策)。
//! 故意不贪多——snapshot 数量小于 ~10 条 review 时才真的能逐条看,超过
//! 这个数会退化成 `cargo insta accept` 一把过的橡皮图章。

use cargo_fresh::display::{
    format_status_line, package_transition, StatusStyle,
};
use cargo_fresh::locale::Language;
use cargo_fresh::models::{PackageInfo, PackageSource, PrebuiltAvailability};

/// 给所有 snapshot 套用同样的 ANSI 剥离 filter,保证 `.snap` 是纯文本
/// 不论 cargo test 跑在 TTY 还是被 `CLICOLOR_FORCE=1` 强开颜色。
fn settings() -> insta::Settings {
    let mut s = insta::Settings::clone_current();
    // ESC [ ... 字母 这一类 ANSI 控制序列全剥
    s.add_filter(r"\x1b\[[0-9;]*[a-zA-Z]", "");
    s
}

fn pkg(name: &str, current: Option<&str>, latest: Option<&str>, source: PackageSource) -> PackageInfo {
    let mut p = PackageInfo::with_source(name.to_string(), current.map(String::from), source);
    p.latest_version = latest.map(String::from);
    p
}

#[test]
fn snapshot_fresh_line() {
    settings().bind(|| {
        insta::assert_snapshot!(format_status_line(
            "Fresh",
            "ripgrep 14.1.1",
            StatusStyle::Dim
        ));
    });
}

#[test]
fn snapshot_updating_line_clean() {
    settings().bind(|| {
        let p = pkg("ripgrep", Some("14.1.0"), Some("14.1.1"), PackageSource::Crates);
        let msg = package_transition(&p, Language::English);
        insta::assert_snapshot!(format_status_line("Updating", &msg, StatusStyle::Ok));
    });
}

/// `--check-prebuilt` 在 Updating 行尾追加 `[prebuilt]`(0.12.0)。
/// 这是个 UX 契约——脚本可以靠它判断"这次升级会从二进制拿还是从源码编",
/// 用户文档也教读这个尾标。位置(行尾、空格分隔)必须稳。
/// 注意：0.10.4 的旧标记 `[binstall: prebuilt]` 已更新为 `[prebuilt]`。
#[test]
fn snapshot_updating_line_with_prebuilt() {
    settings().bind(|| {
        let mut p = pkg("ripgrep", Some("14.1.0"), Some("14.1.1"), PackageSource::Crates);
        p.prebuilt = Some(PrebuiltAvailability::Prebuilt);
        let msg = package_transition(&p, Language::English);
        insta::assert_snapshot!(format_status_line("Updating", &msg, StatusStyle::Ok));
    });
}

/// git source 的 Skip 行——`[git]` 是稳定 marker,`PackageSource::marker()`
/// 出来的。pip 脚本可能 grep `Skip \[git\]` 跳过这类包做汇总。
#[test]
fn snapshot_skip_line_git() {
    settings().bind(|| {
        let p = pkg(
            "my-tool",
            Some("0.1.0"),
            None,
            PackageSource::Git {
                url: "https://example.com/me/my-tool".into(),
                rev: None,
            },
        );
        let msg = package_transition(&p, Language::English);
        insta::assert_snapshot!(format_status_line("Skip", &msg, StatusStyle::Warn));
    });
}

/// `Skip [unknown source]` 是 0.10.1 引入的——之前会被悄悄当成 crates
/// 源去查询失败。marker 写法稳了之后这个 case 必须钉死。
#[test]
fn snapshot_skip_line_unknown_source() {
    settings().bind(|| {
        let p = pkg(
            "weird-tool",
            Some("0.1.0"),
            None,
            PackageSource::Unknown("registry+ssh://internal".into()),
        );
        let msg = package_transition(&p, Language::English);
        insta::assert_snapshot!(format_status_line("Skip", &msg, StatusStyle::Warn));
    });
}

#[test]
fn snapshot_fallback_line() {
    settings().bind(|| {
        insta::assert_snapshot!(format_status_line(
            "Fallback",
            "cargo install --force cargo-fresh --version 1.0.0",
            StatusStyle::Warn
        ));
    });
}

#[test]
fn snapshot_failed_line() {
    settings().bind(|| {
        insta::assert_snapshot!(format_status_line(
            "Failed",
            "cargo-fresh: exit code 1",
            StatusStyle::Err
        ));
    });
}

#[test]
fn snapshot_finished_summary() {
    settings().bind(|| {
        insta::assert_snapshot!(format_status_line(
            "Finished",
            "3 succeeded, 1 failed, in 4.2s",
            StatusStyle::Ok
        ));
    });
}
