//! 自实现的二进制下载器——替代 cargo binstall 子进程。
//!
//! 单元拆分:
//! - `events`:  ProgressEvent / DownloaderError 类型 (无逻辑)
//! - `resolve`: 候选 URL 推导 (纯函数)
//! - `fetch`:   HTTP 流式下载 + sha256
//! - `archive`: tar.gz / zip 解压
//! - `install`: atomic rename + .crates2.json 写
//!
//! 主入口 `download_and_install` 在 Task 7 接入。

pub mod events;
pub mod resolve;
