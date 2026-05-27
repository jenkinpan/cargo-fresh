//! fetch.rs 的 HTTP 集成测试——用 wiremock 模拟 GitHub Release 端点,
//! 验证 HEAD 选择 / 流式下载 / sha256 校验 / cancel 中断的契约。

use cargo_fresh::downloader::events::{DownloaderError, FailureKind, ProgressEvent};
use cargo_fresh::downloader::fetch::fetch;
use cargo_fresh::downloader::resolve::{ArchiveFmt, CandidateUrl};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client() -> reqwest::Client {
    reqwest::Client::builder().build().unwrap()
}

#[tokio::test]
async fn head_404_then_200_picks_second() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/a.tar.gz"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .and(path("/b.tar.gz"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/b.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".as_ref()))
        .mount(&server)
        .await;
    // sha256 endpoint not present — should be tolerated
    Mock::given(method("GET"))
        .and(path("/b.tar.gz.sha256"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let candidates = vec![
        CandidateUrl {
            url: format!("{}/a.tar.gz", server.uri()),
            archive_fmt: ArchiveFmt::TarGz,
        },
        CandidateUrl {
            url: format!("{}/b.tar.gz", server.uri()),
            archive_fmt: ArchiveFmt::TarGz,
        },
    ];

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let r = fetch(&client(), "test", &candidates, &tx, cancel)
        .await
        .expect("ok");
    assert!(r.winning_url.ends_with("/b.tar.gz"));
    let bytes = std::fs::read(&r.archive_path).unwrap();
    assert_eq!(bytes, b"hello");
}

#[tokio::test]
async fn all_head_404_returns_all_urls_failed() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let candidates = vec![CandidateUrl {
        url: format!("{}/x.tar.gz", server.uri()),
        archive_fmt: ArchiveFmt::TarGz,
    }];
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let err = fetch(&client(), "test", &candidates, &tx, cancel)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        DownloaderError::Failed {
            kind: FailureKind::AllUrlsFailed,
            ..
        }
    ));
}

#[tokio::test]
async fn sha256_mismatch_returns_checksum_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/x.tar.gz"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"actual content".as_ref()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/x.tar.gz.sha256"))
        // 64 hex chars, but doesn't match "actual content"
        .respond_with(ResponseTemplate::new(200).set_body_string("a".repeat(64)))
        .mount(&server)
        .await;

    let candidates = vec![CandidateUrl {
        url: format!("{}/x.tar.gz", server.uri()),
        archive_fmt: ArchiveFmt::TarGz,
    }];
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let err = fetch(&client(), "test", &candidates, &tx, cancel)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        DownloaderError::Failed {
            kind: FailureKind::ChecksumMismatch,
            ..
        }
    ));
}

#[tokio::test]
async fn cancel_before_start_returns_cancelled() {
    let server = MockServer::start().await;
    let candidates = vec![CandidateUrl {
        url: format!("{}/x.tar.gz", server.uri()),
        archive_fmt: ArchiveFmt::TarGz,
    }];
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = Arc::new(AtomicBool::new(true));
    let err = fetch(&client(), "test", &candidates, &tx, cancel)
        .await
        .unwrap_err();
    assert!(matches!(err, DownloaderError::Cancelled));
}

#[tokio::test]
async fn downloading_events_emitted_in_order() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let body = vec![0u8; 16_384];
    Mock::given(method("GET"))
        .and(path("/x.tar.gz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body.clone())
                .insert_header("Content-Length", body.len().to_string()),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/x.tar.gz.sha256"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let candidates = vec![CandidateUrl {
        url: format!("{}/x.tar.gz", server.uri()),
        archive_fmt: ArchiveFmt::TarGz,
    }];
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = Arc::new(AtomicBool::new(false));
    fetch(&client(), "test", &candidates, &tx, cancel)
        .await
        .expect("ok");
    drop(tx);

    let mut seen_downloading = 0;
    let mut seen_verifying = false;
    while let Some(e) = rx.recv().await {
        match e {
            ProgressEvent::Downloading { got, total, .. } => {
                seen_downloading += 1;
                assert_eq!(total, Some(16_384));
                assert!(got <= 16_384);
            }
            ProgressEvent::Verifying { .. } => seen_verifying = true,
            _ => {}
        }
    }
    assert!(seen_downloading > 0);
    assert!(seen_verifying);
}
