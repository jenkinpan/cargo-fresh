//! 校验 `--format=json` 真实产生的 `JsonReport` 形状始终匹配
//! `docs/json-schema.json`。schema 是 1.0 对脚本消费者的契约——只许
//! additive 演进，加字段时必须同步更新 schema 文件。本测试在 PR 上把
//! "改了 JSON 结构但忘了改 schema" 这类漂移挡住。
//!
//! 用 `cargo_fresh::models` 暴露的类型直接构造代表性 fixture，覆盖每一种
//! `$defs` 形状（updates_available / fresh / skipped / version_check_errors
//! / results），而不是去跑一次 `cargo fresh` 拿真实输出——同一进程内能稳
//! 定生成所有边界场景，CI runner 上不需要任何预装包。

use cargo_fresh::models::{
    InstallMethod, JsonCheckError, JsonReport, JsonResult, JsonSkipped, JsonSummary,
    JsonUpdateCandidate, PrebuiltAvailability,
};
use jsonschema::Validator;

fn validator() -> Validator {
    let schema_str = include_str!("../docs/json-schema.json");
    let schema: serde_json::Value =
        serde_json::from_str(schema_str).expect("docs/json-schema.json must be valid JSON");
    jsonschema::validator_for(&schema).expect("docs/json-schema.json must be a valid JSON Schema")
}

fn assert_valid(report: &JsonReport, label: &str) {
    let value = serde_json::to_value(report).expect("report must serialize");
    let v = validator();
    if let Err(error) = v.validate(&value) {
        panic!(
            "fixture `{label}` violated docs/json-schema.json: {error}\n\
             payload: {}",
            serde_json::to_string_pretty(&value).unwrap()
        );
    }
}

/// 空快照：没有包、没有更新、没有结果。所有数组都是空、summary 全 0。
/// 这条等同于 CI runner 上一个全新环境跑 `cargo fresh --format=json`
/// 的最小合法输出。
#[test]
fn empty_run_matches_schema() {
    let report = JsonReport {
        schema_version: 2,
        format: "cargo-fresh-v1",
        version: env!("CARGO_PKG_VERSION"),
        include_prerelease: false,
        dry_run: false,
        registry_url: None,
        updates_available: vec![],
        fresh: vec![],
        skipped: vec![],
        version_check_errors: vec![],
        results: vec![],
        summary: JsonSummary {
            checked: 0,
            available: 0,
            selected: 0,
            attempted: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            check_errors: 0,
            duration_ms: 0,
        },
        aborted: false,
    };
    assert_valid(&report, "empty_run");
}

/// 反向校验:把一个合法的 `JsonReport` 序列化后手动捅一个错(违反
/// `format` 的 `const`),确认 validator 真的会报错,而不是被静默放行。
/// 没有这一条,前面三条 `assert_valid` 通过其实只证明了序列化没崩。
#[test]
fn invalid_payload_is_rejected() {
    let mut value = serde_json::to_value(JsonReport {
        schema_version: 2,
        format: "cargo-fresh-v1",
        version: env!("CARGO_PKG_VERSION"),
        include_prerelease: false,
        dry_run: false,
        registry_url: None,
        updates_available: vec![],
        fresh: vec![],
        skipped: vec![],
        version_check_errors: vec![],
        results: vec![],
        summary: JsonSummary {
            checked: 0,
            available: 0,
            selected: 0,
            attempted: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            check_errors: 0,
            duration_ms: 0,
        },
        aborted: false,
    })
    .unwrap();
    value["format"] = serde_json::Value::String("not-the-discriminator".into());

    let v = validator();
    assert!(
        v.validate(&value).is_err(),
        "validator should reject a payload whose `format` discriminator is wrong"
    );
}

/// 覆盖每一种 `$defs` 形状的"满"快照：
/// - `updates_available` 含一个 prerelease=false + prebuilt=prebuilt 与
///   一个 prerelease=true + prebuilt=null
/// - `fresh` 含一个名字
/// - `skipped` 覆盖 git/path/unknown 三种 reason_code
/// - `version_check_errors` 含一个 not_found 一个 unavailable
/// - `results` 含一个 success 一个 failure
/// - `registry_url` 是 Some
/// - `dry_run` / `include_prerelease` / `aborted` 都翻成 true
#[test]
fn full_run_matches_schema() {
    let report = JsonReport {
        schema_version: 2,
        format: "cargo-fresh-v1",
        version: env!("CARGO_PKG_VERSION"),
        include_prerelease: true,
        dry_run: true,
        registry_url: Some("https://mirror.example.com"),
        updates_available: vec![
            JsonUpdateCandidate {
                name: "ripgrep",
                current: Some("14.1.0"),
                latest: "14.1.1",
                source: "crates",
                prerelease: false,
                prebuilt: Some(PrebuiltAvailability::Prebuilt.kind_str()),
            },
            JsonUpdateCandidate {
                name: "cargo-fresh",
                current: None,
                latest: "1.0.0-rc.1",
                source: "crates",
                prerelease: true,
                prebuilt: None,
            },
        ],
        fresh: vec!["bat"],
        skipped: vec![
            JsonSkipped {
                name: "my-tool",
                source: "git",
                reason_code: "git_source",
                reason: "non-crates source: version check skipped",
            },
            JsonSkipped {
                name: "local-tool",
                source: "path",
                reason_code: "path_source",
                reason: "non-crates source: version check skipped",
            },
            JsonSkipped {
                name: "weird-tool",
                source: "unknown",
                reason_code: "unknown_source",
                reason: "non-crates source: version check skipped",
            },
        ],
        version_check_errors: vec![
            JsonCheckError {
                name: "ghost-crate",
                kind: "not_found",
                error: "package not found on the configured registry",
            },
            JsonCheckError {
                name: "flaky-crate",
                kind: "unavailable",
                error: "sparse index request failed after 1 retry",
            },
        ],
        results: vec![
            JsonResult {
                name: "ripgrep",
                old_version: Some("14.1.0"),
                new_version: Some("14.1.1"),
                success: true,
                install_method: InstallMethod::Downloader.json_str(),
            },
            JsonResult {
                name: "cargo-fresh",
                old_version: Some("0.10.6"),
                new_version: None,
                success: false,
                // 失败/中止 → Unknown → json_str() == None → JSON null
                install_method: InstallMethod::Unknown.json_str(),
            },
        ],
        summary: JsonSummary {
            checked: 6,
            available: 2,
            selected: 2,
            attempted: 2,
            succeeded: 1,
            failed: 1,
            skipped: 3,
            check_errors: 2,
            duration_ms: 1234,
        },
        aborted: true,
    };
    assert_valid(&report, "full_run");
}

/// 覆盖 `prebuilt` 字段剩下两个枚举值——`source` 与 `unknown`。
/// 与 `full_run` 拆开是为了让"哪条 fixture 命中了 schema 哪条规则"在失败
/// 时一目了然。
#[test]
fn prebuilt_variants_match_schema() {
    for kind in [PrebuiltAvailability::Source, PrebuiltAvailability::Unknown] {
        let report = JsonReport {
            schema_version: 2,
            format: "cargo-fresh-v1",
            version: env!("CARGO_PKG_VERSION"),
            include_prerelease: false,
            dry_run: false,
            registry_url: None,
            updates_available: vec![JsonUpdateCandidate {
                name: "some-crate",
                current: Some("1.0.0"),
                latest: "1.0.1",
                source: "crates",
                prerelease: false,
                prebuilt: Some(kind.kind_str()),
            }],
            fresh: vec![],
            skipped: vec![],
            version_check_errors: vec![],
            results: vec![],
            summary: JsonSummary {
                checked: 1,
                available: 1,
                selected: 0,
                attempted: 0,
                succeeded: 0,
                failed: 0,
                skipped: 0,
                check_errors: 0,
                duration_ms: 7,
            },
            aborted: false,
        };
        assert_valid(&report, &format!("prebuilt_{}", kind.kind_str()));
    }
}
