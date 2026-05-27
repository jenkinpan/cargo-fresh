//! install.rs 的 tempdir 集成测试——用 isolated CARGO_HOME 验证 atomic
//! rename + .crates2.json 更新, 不污染真实 ~/.cargo。

use cargo_fresh::downloader::install::install_binary;
use std::io::Write;

#[test]
fn install_binary_atomically_into_isolated_cargo_home() {
    let cargo_home = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(cargo_home.path().join("bin")).unwrap();
    // .crates2.json fixture: 已存在 ripgrep 14.1.1
    let crates2 = r#"{
        "installs": {
            "ripgrep 14.1.1 (registry+https://github.com/rust-lang/crates.io-index)": {
                "version_req": null,
                "bins": ["rg"],
                "features": [],
                "all_features": false,
                "no_default_features": false,
                "profile": "release",
                "target": "x86_64-apple-darwin",
                "rustc": "rustc 1.0",
                "metadata": "0"
            }
        }
    }"#;
    std::fs::write(cargo_home.path().join(".crates2.json"), crates2).unwrap();

    let src = cargo_home.path().join("src-binary");
    let mut f = std::fs::File::create(&src).unwrap();
    f.write_all(b"#!/bin/sh\necho new rg\n").unwrap();

    // Point install_binary at this tempdir
    let prev = std::env::var("CARGO_HOME").ok();
    std::env::set_var("CARGO_HOME", cargo_home.path());

    let dest = install_binary(&src, "rg", "14.1.2").expect("install ok");

    // Restore env
    match prev {
        Some(v) => std::env::set_var("CARGO_HOME", v),
        None => std::env::remove_var("CARGO_HOME"),
    }

    // Verify binary placed
    assert!(dest.exists());
    assert_eq!(dest, cargo_home.path().join("bin").join("rg"));

    // Verify .crates2.json updated
    let body = std::fs::read_to_string(cargo_home.path().join(".crates2.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    let keys: Vec<String> = v["installs"].as_object().unwrap().keys().cloned().collect();
    assert!(
        keys.iter().any(|k| k.starts_with("ripgrep 14.1.2 ")),
        "expected updated key, got: {keys:?}"
    );
    assert!(
        !keys.iter().any(|k| k.starts_with("ripgrep 14.1.1 ")),
        "old key should be gone: {keys:?}"
    );

    // Verify no leftover .cargo-fresh-*.tmp
    let leftover: Vec<_> = std::fs::read_dir(cargo_home.path().join("bin"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".cargo-fresh-")
        })
        .collect();
    assert!(leftover.is_empty(), "tmp file leaked: {leftover:?}");
}
