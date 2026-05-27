//! HTTP 下载 + sha256 校验。
//!
//! - 对一组候选 URL 依次 HEAD, 选第一个 2xx 的胜出。
//! - 对胜出 URL 流式 GET, 每 chunk 后发 `Downloading` 事件并检查 cancel。
//! - 并发尝试 GET `{url}.sha256` (容忍 404), 校验本地文件 sha256。
//! - 整个流程的临时文件在返回的 `FetchedArchive` 持有的 TempDir 里——
//!   caller 用完丢弃即可。

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

use crate::downloader::events::{DownloaderError, FailureKind, ProgressEvent};
use crate::downloader::resolve::CandidateUrl;

#[derive(Debug)]
pub struct FetchedArchive {
    pub temp_dir: tempfile::TempDir,
    pub archive_path: PathBuf,
    pub winning_url: String,
}

/// 主下载入口。`name` 仅用于事件文案 (channel 消费方按 name 分组渲染)。
pub async fn fetch(
    client: &reqwest::Client,
    name: &str,
    candidates: &[CandidateUrl],
    events: &tokio::sync::mpsc::UnboundedSender<ProgressEvent>,
    cancel: Arc<AtomicBool>,
) -> Result<FetchedArchive, DownloaderError> {
    if cancel.load(Ordering::SeqCst) {
        return Err(DownloaderError::Cancelled);
    }

    // 1. HEAD 探测——选第一个 2xx
    let winner = {
        let mut found = None;
        for c in candidates {
            if cancel.load(Ordering::SeqCst) {
                return Err(DownloaderError::Cancelled);
            }
            let _ = events.send(ProgressEvent::UrlCandidate {
                name: name.to_string(),
                url: c.url.clone(),
            });
            match client.head(&c.url).send().await {
                Ok(r) if r.status().is_success() => {
                    found = Some(c);
                    break;
                }
                _ => continue,
            }
        }
        found.ok_or_else(|| DownloaderError::Failed {
            kind: FailureKind::AllUrlsFailed,
            source: anyhow!("no candidate URL returned 2xx"),
        })?
    };

    // 2. 准备临时文件
    let temp_dir = tempfile::tempdir().map_err(|e| DownloaderError::Failed {
        kind: FailureKind::DownloadInterrupted,
        source: anyhow!(e).context("mkdir tempdir for download"),
    })?;
    let filename = winner
        .url
        .rsplit('/')
        .next()
        .unwrap_or("download.bin")
        .to_string();
    let archive_path = temp_dir.path().join(&filename);

    // 3. GET 流式下载
    let resp = client.get(&winner.url).send().await.map_err(|e| {
        DownloaderError::Failed {
            kind: FailureKind::DownloadInterrupted,
            source: anyhow!(e).context("GET archive"),
        }
    })?;
    let total = resp.content_length();
    let mut stream = resp.bytes_stream();

    let mut file = tokio::fs::File::create(&archive_path)
        .await
        .map_err(|e| DownloaderError::Failed {
            kind: FailureKind::DownloadInterrupted,
            source: anyhow!(e).context("create archive file"),
        })?;

    let mut got: u64 = 0;
    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            return Err(DownloaderError::Cancelled);
        }
        let bytes = chunk.map_err(|e| DownloaderError::Failed {
            kind: FailureKind::DownloadInterrupted,
            source: anyhow!(e).context("read chunk"),
        })?;
        file.write_all(&bytes)
            .await
            .map_err(|e| DownloaderError::Failed {
                kind: FailureKind::DownloadInterrupted,
                source: anyhow!(e).context("write chunk"),
            })?;
        got += bytes.len() as u64;
        let _ = events.send(ProgressEvent::Downloading {
            name: name.to_string(),
            got,
            total,
        });
    }
    file.flush()
        .await
        .map_err(|e| DownloaderError::Failed {
            kind: FailureKind::DownloadInterrupted,
            source: anyhow!(e).context("flush"),
        })?;
    drop(file);

    // 4. sha256 校验 (best-effort)
    let _ = events.send(ProgressEvent::Verifying {
        name: name.to_string(),
    });
    if let Ok(resp) = client.get(format!("{}.sha256", winner.url)).send().await {
        if resp.status().is_success() {
            let expected_hex = resp.text().await.ok().and_then(parse_sha256_hex);
            if let Some(expected) = expected_hex {
                let actual = compute_sha256(&archive_path)
                    .await
                    .map_err(|e| DownloaderError::Failed {
                        kind: FailureKind::ChecksumMismatch,
                        source: e.context("compute sha256"),
                    })?;
                if actual != expected {
                    return Err(DownloaderError::Failed {
                        kind: FailureKind::ChecksumMismatch,
                        source: anyhow!(
                            "expected sha256={expected}, got sha256={actual}"
                        ),
                    });
                }
            }
        }
    }

    Ok(FetchedArchive {
        temp_dir,
        archive_path,
        winning_url: winner.url.clone(),
    })
}

/// `.sha256` 文件通常是 "abc123... filename" 或纯 hex——抽出 hex 部分。
fn parse_sha256_hex(body: String) -> Option<String> {
    let token = body.split_whitespace().next()?.to_lowercase();
    if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token)
    } else {
        None
    }
}

async fn compute_sha256(path: &std::path::Path) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let mut f = tokio::fs::File::open(path).await.context("open for sha256")?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let n = f.read(&mut buf).await.context("read for sha256")?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sha256_strict_hex_64() {
        assert_eq!(
            parse_sha256_hex("a".repeat(64) + " file.tar.gz\n"),
            Some("a".repeat(64))
        );
    }

    #[test]
    fn parse_sha256_rejects_wrong_length() {
        assert_eq!(parse_sha256_hex("deadbeef file.tar.gz".into()), None);
    }

    #[test]
    fn parse_sha256_rejects_non_hex() {
        assert_eq!(parse_sha256_hex("z".repeat(64)), None);
    }
}
