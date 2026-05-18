# Security Policy

## 报告漏洞

如果你发现 cargo-fresh 的安全问题，请**不要**直接提 public issue。

请通过以下渠道之一私下联系维护者：

- GitHub Security Advisories（推荐）：在 <https://github.com/jenkinpan/cargo-fresh/security/advisories/new> 提交
- Email: 见 `Cargo.toml` 中 maintainer 关联的 GitHub profile

请在报告中包含：

1. 漏洞类型与影响范围
2. 复现步骤 / PoC（命令行 + 必要的环境配置）
3. 受影响的版本范围
4. 你建议的缓解方案（可选）

我会在 72 小时内回复并确认。修复发布后会在 GitHub Security Advisories 公开披露并标注 CVE（如适用）。

## 支持范围

| 版本 | 状态 |
|------|------|
| 0.10.x | 当前 |
| < 0.10.0 | 不再支持，请升级 |
| 1.x（未发布） | 计划中 |

## 信任边界

cargo-fresh 是一个**本地运行的 CLI**，会：

- 读取 `~/.cargo/.crates.toml` / `cargo install --list`
- 通过 HTTPS 访问 `index.crates.io`（或用户配置的镜像）
- 调用 `cargo install` / `cargo binstall` 子进程，**这些子进程会下载并编译/运行任意 crate 代码**

特别注意：

- cargo-fresh **不验证 crate 来源信任性**——升级前请用 `--dry-run` 看清楚要装什么版本
- `--registry-url` 直接成为 HTTP 请求 URL；指向恶意服务器会拿到你装了哪些 crate 的指纹
- `cargo binstall` 会从 GitHub Releases 拉预编译二进制；这一步的信任模型由 cargo-binstall 决定，不由 cargo-fresh

## 非漏洞

以下情况不视为安全问题，请走 Issues 而非 Security：

- 升级到含 yank 的版本（用 `--dry-run` 自查）
- cargo-fresh 调用的子进程崩溃但 cargo-fresh 自身正常退出
- 中英文文案错别字 / UI 颜色对比度问题（属于 UX bug）
