//! HTTP-level tests for downloader::probe.
//!
//! Uses wiremock to stand up a local server. Builds a Vec<String> of URLs
//! pointing at that server and feeds them straight into the testable inner
//! function `probe_with_candidates`, so we exercise the real HEAD-loop +
//! timeout behavior without depending on github.com.

use cargo_fresh::downloader::probe::probe_with_candidates;
use cargo_fresh::models::PrebuiltAvailability;

use std::time::Duration;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn make_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("build reqwest client")
}

#[tokio::test]
async fn returns_prebuilt_when_any_head_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let urls = vec![format!("{}/anything.tar.gz", server.uri())];
    let result = probe_with_candidates(&make_client(), &urls).await;
    assert_eq!(result, PrebuiltAvailability::Prebuilt);
}

#[tokio::test]
async fn returns_source_when_all_404() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let urls = vec![
        format!("{}/a.tar.gz", server.uri()),
        format!("{}/b.zip", server.uri()),
        format!("{}/c.tar.gz", server.uri()),
    ];
    let result = probe_with_candidates(&make_client(), &urls).await;
    assert_eq!(result, PrebuiltAvailability::Source);
}

#[tokio::test]
async fn returns_unknown_when_all_503() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let urls = vec![format!("{}/a.tar.gz", server.uri())];
    let result = probe_with_candidates(&make_client(), &urls).await;
    assert_eq!(result, PrebuiltAvailability::Unknown);
}

#[tokio::test]
async fn returns_unknown_on_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(8)))
        .mount(&server)
        .await;
    let urls = vec![format!("{}/slow.tar.gz", server.uri())];
    let result = probe_with_candidates(&make_client(), &urls).await;
    assert_eq!(result, PrebuiltAvailability::Unknown);
}

#[tokio::test]
async fn returns_source_when_no_candidates() {
    let urls: Vec<String> = vec![];
    let result = probe_with_candidates(&make_client(), &urls).await;
    // Empty candidate list means resolve couldn't produce URLs (non-GitHub repo,
    // etc). Treat as Source — caller will fall back to cargo install.
    assert_eq!(result, PrebuiltAvailability::Source);
}
