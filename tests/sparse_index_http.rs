//! `sparse_index::fetch_latest` 的网络层契约测试。
//!
//! 用 `wiremock` 起一个本地 HTTP，把 `base_url` 指过去，覆盖：
//! - 200 + 合法 body  → 返回 LatestVersions
//! - 200 + 空 body    → 返回空 LatestVersions（不报错）
//! - 404              → 立刻 Err，不重试（4xx 不重试是显式契约）
//! - 5xx              → 重试一次，仍失败则 Err
//! - 头一次 5xx，第二次 200 → 重试成功
//!
//! 跑这些不联网，纯粹验证客户端行为。

use cargo_fresh::package::sparse_index::{fetch_latest, SparseIndexError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("cargo-fresh-test")
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap()
}

#[tokio::test]
async fn success_returns_parsed_versions() {
    let server = MockServer::start().await;
    let body = r#"{"name":"ripgrep","vers":"13.0.0","yanked":false}
{"name":"ripgrep","vers":"14.1.1","yanked":false}
"#;
    Mock::given(method("GET"))
        .and(path("/ri/pg/ripgrep"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .expect(1)
        .mount(&server)
        .await;

    let v = fetch_latest(&client(), &server.uri(), "ripgrep").await.unwrap();
    assert_eq!(v.stable.as_deref(), Some("14.1.1"));
}

#[tokio::test]
async fn not_found_is_not_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ca/rg/cargo-nonexistent"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1) // 4xx 必须只请求一次
        .mount(&server)
        .await;

    let err = fetch_latest(&client(), &server.uri(), "cargo-nonexistent")
        .await
        .unwrap_err();
    assert!(matches!(err, SparseIndexError::NotFound), "err = {err:?}");
}

#[tokio::test]
async fn server_error_is_retried_once_then_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ri/pg/ripgrep"))
        .respond_with(ResponseTemplate::new(503))
        .expect(2) // MAX_ATTEMPTS = 2
        .mount(&server)
        .await;

    let err = fetch_latest(&client(), &server.uri(), "ripgrep")
        .await
        .unwrap_err();
    match err {
        SparseIndexError::Unavailable(e) => {
            assert!(e.to_string().contains("503"), "inner = {e}");
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[tokio::test]
async fn server_error_then_success_recovers() {
    let server = MockServer::start().await;
    // wiremock 按注册顺序匹配——先注册 503 限定 1 次，再注册 200 兜底
    Mock::given(method("GET"))
        .and(path("/ri/pg/ripgrep"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/ri/pg/ripgrep"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"name":"ripgrep","vers":"14.0.0","yanked":false}"#),
        )
        .mount(&server)
        .await;

    let v = fetch_latest(&client(), &server.uri(), "ripgrep").await.unwrap();
    assert_eq!(v.stable.as_deref(), Some("14.0.0"));
}

#[tokio::test]
async fn empty_body_returns_empty_versions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ca/rg/cargo-fresh"))
        .respond_with(ResponseTemplate::new(200).set_body_string(""))
        .expect(1)
        .mount(&server)
        .await;

    let v = fetch_latest(&client(), &server.uri(), "cargo-fresh").await.unwrap();
    assert!(v.stable.is_none() && v.prerelease.is_none());
}

#[tokio::test]
async fn check_package_updates_records_unavailable_error() {
    use cargo_fresh::models::{CheckErrorKind, PackageInfo, PackageSource};
    use cargo_fresh::package::check_package_updates;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let mut packages = vec![PackageInfo::with_source(
        "ripgrep".into(),
        Some("14.0.0".into()),
        PackageSource::Crates,
    )];
    // no_fallback = true 跳过 cargo search 慢路径，保证离线确定性
    check_package_updates(&mut packages, false, false, Some(server.uri()), true)
        .await
        .unwrap();

    let err = packages[0].check_error.as_ref().expect("check_error set");
    assert_eq!(err.kind, CheckErrorKind::Unavailable);
    assert!(packages[0].latest_version.is_none());
}
