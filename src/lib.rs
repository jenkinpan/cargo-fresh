//! cargo-fresh 的 lib 入口。
//!
//! 主要目的是让 `tests/` 下的集成测试能直接调内部模块（如
//! `cargo_fresh::package::sparse_index::fetch_latest`），而不必通过
//! 启动子进程的方式去触达。bin 与 lib 共用同一份模块树。

pub mod cli;
pub mod display;
pub mod errors;
pub mod locale;
pub mod models;
pub mod package;
pub mod updater;
