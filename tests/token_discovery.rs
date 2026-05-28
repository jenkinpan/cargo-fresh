//! Token-discovery tests need env var isolation — `#[serial]` ensures they
//! never run concurrently with each other or with other env-touching tests.

use cargo_fresh::downloader::token::discover_token_uncached;
use serial_test::serial;

#[test]
#[serial]
fn github_token_wins_over_gh_token() {
    std::env::set_var("GITHUB_TOKEN", "from-github");
    std::env::set_var("GH_TOKEN", "from-gh");
    assert_eq!(discover_token_uncached().as_deref(), Some("from-github"));
    std::env::remove_var("GITHUB_TOKEN");
    std::env::remove_var("GH_TOKEN");
}

#[test]
#[serial]
fn gh_token_fallback_when_github_token_missing() {
    std::env::remove_var("GITHUB_TOKEN");
    std::env::set_var("GH_TOKEN", "from-gh");
    assert_eq!(discover_token_uncached().as_deref(), Some("from-gh"));
    std::env::remove_var("GH_TOKEN");
}

#[test]
#[serial]
fn empty_env_var_is_ignored() {
    std::env::set_var("GITHUB_TOKEN", "");
    std::env::remove_var("GH_TOKEN");
    // empty string is a misconfiguration — should NOT be treated as a valid token
    // (otherwise we'd send "Authorization: Bearer " and get 401 on every request)
    let result = discover_token_uncached();
    // result may be None or Some("from gh subprocess if available") — we only
    // require that it's not Some("")
    assert_ne!(result.as_deref(), Some(""));
    std::env::remove_var("GITHUB_TOKEN");
}
