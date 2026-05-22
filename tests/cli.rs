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
        "--check-binstall",
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

#[test]
fn man_subcommand_emits_roff() {
    // `cargo fresh man` 输出 troff/roff 格式的 man page 到 stdout，
    // 应包含 .TH 头、NAME/SYNOPSIS/OPTIONS 段，以及若干核心标志名
    let assert = bin().arg("man").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(out.starts_with(".ie") || out.contains(".TH"), "man output missing roff header:\n{}", &out[..out.len().min(200)]);
    for marker in [".TH cargo-fresh", ".SH NAME", ".SH SYNOPSIS", ".SH OPTIONS"] {
        assert!(out.contains(marker), "man output missing {marker}");
    }
    for flag in ["\\-\\-dry\\-run", "\\-\\-format", "\\-\\-include\\-prerelease"] {
        assert!(out.contains(flag), "man output missing {flag}");
    }
}

#[test]
fn json_mode_keeps_stdout_clean() {
    // --format=json 的合约：stdout 只有一行可解析的 JSON；状态行/进度全部走 stderr
    let assert = bin()
        .args(["--batch", "--dry-run", "--format=json", "--filter=__nonexistent_pkg_xyz__"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let trimmed = out.trim();
    assert!(
        trimmed.starts_with('{') && trimmed.ends_with('}'),
        "stdout should be a single JSON object, got:\n{out}"
    );
    assert!(
        trimmed.lines().count() == 1,
        "stdout should be exactly one line of JSON, got {} lines:\n{out}",
        trimmed.lines().count()
    );
    assert!(
        trimmed.contains("\"schema_version\":1"),
        "JSON missing schema_version=1:\n{out}"
    );
}

#[test]
fn json_mode_emits_new_contract_fields() {
    // 1.0 合约新增字段（schema_version=1 增量）必须始终出现在 JSON 报告中，
    // 即便本次运行匹配不到任何包——下游脚本据此可无条件解析这些键
    let assert = bin()
        .args(["--batch", "--dry-run", "--format=json", "--filter=__nonexistent_pkg_xyz__"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let trimmed = out.trim();
    for key in [
        "\"version_check_errors\":",
        "\"selected\":",
        "\"attempted\":",
        "\"check_errors\":",
    ] {
        assert!(trimmed.contains(key), "JSON missing {key}\n{out}");
    }
}

#[test]
fn no_color_env_strips_ansi_from_stderr() {
    // NO_COLOR=1 + 非 TTY stderr：anstream 应该把 ANSI 序列裁干净
    let assert = bin()
        .env("NO_COLOR", "1")
        .args(["--batch", "--dry-run", "--filter=__nonexistent_pkg_xyz__"])
        .assert()
        .success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        !err.contains("\x1b["),
        "stderr should be ANSI-free under NO_COLOR=1, got:\n{err}"
    );
    // 但状态行的文字内容应仍在
    assert!(err.contains("Checking"), "expected 'Checking' verb in stderr:\n{err}");
}

#[test]
fn clicolor_force_keeps_ansi_when_redirected() {
    // CLICOLOR_FORCE=1：即便 stderr 不是 TTY，也应保留颜色码
    let assert = bin()
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .args(["--batch", "--dry-run", "--filter=__nonexistent_pkg_xyz__"])
        .assert()
        .success();
    let err = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        err.contains("\x1b["),
        "stderr should contain ANSI escapes under CLICOLOR_FORCE=1, got:\n{err}"
    );
}

#[test]
fn non_json_mode_keeps_status_off_stdout() {
    // 非 JSON 模式下，status 行应全部到 stderr；stdout 必须为空
    let assert = bin()
        .args(["--batch", "--dry-run", "--filter=__nonexistent_pkg_xyz__"])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        out.trim().is_empty(),
        "stdout should be empty in non-JSON mode, got:\n{out}"
    );
}
