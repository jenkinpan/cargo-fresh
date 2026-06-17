//! HTTP integration tests for github_api. wiremock stands up a fake
//! api.github.com so we don't depend on real rate limit / network.

use cargo_fresh::downloader::github_api::{
    fetch_release_assets, match_winning_asset, parse_owner_repo, GithubApiError, ReleaseAsset,
};

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
        .respond_with(
            ResponseTemplate::new(403)
                .insert_header("x-ratelimit-remaining", "0"),
        )
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    assert!(matches!(result, Err(GithubApiError::RateLimited)));
}

#[tokio::test]
async fn returns_rate_limited_on_401() {
    // 401 (bad/expired token) shares the RateLimited arm with 403/429 — all three
    // mean "give up on the API and fall back", so the caller routes them identically.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    assert!(matches!(result, Err(GithubApiError::RateLimited)));
}

#[tokio::test]
async fn returns_parse_error_on_unexpected_status() {
    // 5xx (and any other unmapped status) → Parse("unexpected status N"). Distinct
    // from NotFound/RateLimited so a server-side blip doesn't get silently treated
    // as "tag doesn't exist".
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    match result {
        Err(GithubApiError::Parse(msg)) => assert!(
            msg.contains("500"),
            "Parse message should mention the status code, got: {msg}"
        ),
        other => panic!("expected Parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn returns_parse_error_on_malformed_200_body() {
    // 200 with a body that isn't a ReleaseResponse → Parse, not a panic / empty list.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{ not valid json"))
        .mount(&server)
        .await;
    let client = reqwest::Client::new();
    let result = fetch_release_assets(&client, &server.uri(), "x", "y", "v1", None).await;
    assert!(matches!(result, Err(GithubApiError::Parse(_))));
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

fn asset(name: &str) -> ReleaseAsset {
    ReleaseAsset {
        name: name.to_string(),
        browser_download_url: format!("https://example.com/{name}"),
    }
}

#[test]
fn match_winning_asset_finds_exact_name() {
    let assets = [
        asset("ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz"),
        asset("ripgrep-15.1.0-aarch64-apple-darwin.tar.gz"),
    ];
    let expected = vec!["ripgrep-15.1.0-aarch64-apple-darwin.tar.gz".to_string()];
    let winner = match_winning_asset(&assets, &expected).expect("should match");
    assert_eq!(winner.name, "ripgrep-15.1.0-aarch64-apple-darwin.tar.gz");
}

#[test]
fn match_winning_asset_returns_none_when_no_candidate_matches() {
    let assets = [asset("ripgrep-15.1.0-x86_64-pc-windows-msvc.zip")];
    let expected = vec!["ripgrep-15.1.0-aarch64-apple-darwin.tar.gz".to_string()];
    assert!(match_winning_asset(&assets, &expected).is_none());
}

#[test]
fn match_winning_asset_returns_first_asset_order_not_candidate_order() {
    // The winner is the first *asset* whose name is in the candidate set — asset
    // order wins, not candidate order. Here both assets are candidates; the one
    // appearing first in `assets` must be returned.
    let assets = [asset("first.tar.gz"), asset("second.tar.gz")];
    let expected = vec!["second.tar.gz".to_string(), "first.tar.gz".to_string()];
    let winner = match_winning_asset(&assets, &expected).expect("should match");
    assert_eq!(winner.name, "first.tar.gz");
}

#[test]
fn match_winning_asset_empty_inputs_yield_none() {
    let assets: [ReleaseAsset; 0] = [];
    assert!(match_winning_asset(&assets, &["x".to_string()]).is_none());
    let assets = [asset("a.tar.gz")];
    assert!(match_winning_asset(&assets, &[]).is_none());
}
