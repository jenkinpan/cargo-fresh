//! 极小的 crates.io API client——只拿 `repository` 字段。
//!
//! sparse index 不带 repo URL, 必须走另一条接口:
//!   GET https://crates.io/api/v1/crates/{name}
//! 拿回 JSON 里的 `crate.repository`。失败一律返回 None, 让调用方走
//! Phase 2 fallback——不让 API 故障变成更新失败。

pub async fn fetch_repo_url(client: &reqwest::Client, name: &str) -> Option<String> {
    let url = format!("https://crates.io/api/v1/crates/{name}");
    // crates.io 强制要求 User-Agent
    let resp = client
        .get(url)
        .header(
            "User-Agent",
            "cargo-fresh (https://github.com/jenkinpan/cargo-fresh)",
        )
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    let body: serde_json::Value = serde_json::from_str(&text).ok()?;
    body.get("crate")?
        .get("repository")?
        .as_str()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    // 真实 API 测试单独放在 tests/ 用 #[ignore], 本模块只编译时验证
}
