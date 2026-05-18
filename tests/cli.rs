//! CLI 集成测试：通过 `assert_cmd` 启动真实二进制，验证关键命令行入口的行为。
//!
//! 这里只做"对外契约"层面的检查——版本号、帮助文本里的关键标志、completion 能产出非空脚本。
//! 具体输出文本不做 byte-for-byte 快照，避免改一个标点就要更新一堆 snapshot。

use assert_cmd::Command;
use predicates::prelude::*;

fn bin() -> Command {
    Command::cargo_bin("cargo-fresh").expect("binary built")
}

#[test]
fn version_flag_prints_crate_version() {
    bin()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_core_flags() {
    let assert = bin().arg("--help").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    for flag in [
        "--dry-run",
        "--batch",
        "--filter",
        "--exclude",
        "--include-prerelease",
        "--registry-url",
        "--format",
        "--no-cargo-search-fallback",
        "--install-binstall",
    ] {
        assert!(out.contains(flag), "help missing {flag}\n--- help ---\n{out}");
    }
}

#[test]
fn cargo_subcommand_form_help_works() {
    // 以 `cargo fresh` 形式启动时，argv[1] 是 "fresh"——CLI 必须吃掉这个子命令名
    bin()
        .args(["fresh", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--dry-run"));
}

#[test]
fn completion_bash_emits_script() {
    let assert = bin().args(["completion", "bash"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        out.contains("_cargo-fresh") || out.contains("complete -F"),
        "bash completion looks empty:\n{out}"
    );
}

#[test]
fn completion_fish_emits_script() {
    let assert = bin().args(["completion", "fish"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(out.contains("complete -c"), "fish completion looks empty:\n{out}");
}
