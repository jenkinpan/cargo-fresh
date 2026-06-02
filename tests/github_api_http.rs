//! HTTP integration tests for github_api. wiremock stands up a fake
//! api.github.com so we don't depend on real rate limit / network.

use cargo_fresh::downloader::github_api::{fetch_release_assets, parse_owner_repo, GithubApiError};

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn assets_json() -> serde_json::Value {
    serde_json::json!({
        "tag_name": "15.1.0",
        "assets": [
            {
                "name": "ripgrep-15.1.0-aarch64-apple-darwin.tar.gz",
                "browser_download_url": "https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-aarch64-apple-darwin.tar.gz"
            },
            {
                "name": "ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz",
                "browser_download_url": "https://github.com/BurntSushi/ripgrep/releases/download/15.1.0/ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz"
            }
        ]
    })
}

#[tokio::test]
async fn fetches_assets_on_200() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/BurntSushi/ripgrep/releases/tags/15.1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(assets_json()))
        .mount(&server)
        .await;

    let client = reqwest::Client::new();
    let result = fetch_release_assets(
        &client,
        &server.uri(),
        "BurntSushi",
        "ripgrep",
        "15.1.0",
        None,
    )
    .await
    .expect("ok");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].name, "ripgrep-15.1.0-aarch64-apple-darwin.tar.gz");
}

#[tokio::test]
async fn returns_not_found_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    assert!(matches!(result, Err(GithubApiError::NotFound)));
}

#[tokio::test]
async fn returns_rate_limited_on_403() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403).insert_header("x-ratelimit-remaining", "0"))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    assert!(matches!(result, Err(GithubApiError::RateLimited)));
}

#[tokio::test]
async fn returns_rate_limited_on_429() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    assert!(matches!(result, Err(GithubApiError::RateLimited)));
}

#[tokio::test]
async fn auth_header_included_when_token_set() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer test-token-xyz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(assets_json()))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(
        &client,
        &server.uri(),
        "BurntSushi",
        "ripgrep",
        "15.1.0",
        Some("test-token-xyz"),
    )
    .await;
    assert!(result.is_ok(), "auth header should have been sent");
}

#[test]
fn parse_owner_repo_extracts_basic() {
    assert_eq!(
        parse_owner_repo("https://github.com/BurntSushi/ripgrep"),
        Some(("BurntSushi".to_string(), "ripgrep".to_string()))
    );
}

#[test]
fn parse_owner_repo_strips_dot_git() {
    assert_eq!(
        parse_owner_repo("https://github.com/BurntSushi/ripgrep.git"),
        Some(("BurntSushi".to_string(), "ripgrep".to_string()))
    );
}

#[test]
fn parse_owner_repo_strips_trailing_slash() {
    assert_eq!(
        parse_owner_repo("https://github.com/BurntSushi/ripgrep/"),
        Some(("BurntSushi".to_string(), "ripgrep".to_string()))
    );
}

#[test]
fn parse_owner_repo_rejects_non_github() {
    assert_eq!(parse_owner_repo("https://gitlab.com/x/y"), None);
}
