# ROADMAP

Post-0.9.14 work plan. P0 items are tracked separately and land first as 0.10.0; P1 lands as 1.0.0-rc.1; P2 is 1.x territory.

The original review (with rationale, current-state references, and risk notes for every item) lives in the chat transcript that produced this file. This file is the durable checklist.

## P0 — block 1.0 (status: in progress)

Tracked via TaskList; see commit history `feat(P0-*)`, `fix(P0-*)`.

1. **P0-1** Unify completion generation with `CommandFactory`
2. **P0-2** Handle SIGINT and abort in-flight updates cleanly
3. **P0-3** Respect cargo registry sparse mirror
4. **P0-4** Stop surfacing prereleases when `--include-prerelease` is off (BREAKING)
5. **P0-5** Add `--format=json` and document exit codes
6. **P0-6** Declare `rust-version` (MSRV) and enforce in CI

## P1 — expected for 1.0.0-rc.1

### P1-1 Push blocking IO out of async fns
`get_installed_packages` / `get_installed_version` / `update_package` 内的 `std::process::Command::output()` 是同步阻塞，跑在 `#[tokio::main]` runtime 里。当前串行所以不爆；P0-2 引入并发后会咬人。
- 换 `tokio::process::Command` 或 `tokio::task::spawn_blocking`。
- `tokio = { features = ["full"] }` 收紧到 `["rt-multi-thread","macros","signal","process","time","sync"]`，编译时间显著下降。

### P1-2 `cargo search` fallback 改 opt-out
- 加 `--no-cargo-search-fallback` 或 `CARGO_FRESH_NO_FALLBACK=1`。
- `--verbose` 模式下打印 `   Fallback cargo search (slow path) for {name}`。

### P1-3 `verbose` 走裸 `println!`，违反"全部通过 status*"约定
- `package/mod.rs::check_package_updates` 里三处 `println!` 换 `status_dim("Check", ...)`。
- 引入 `is_terminal` 在非 TTY 时禁用 spinner。

### P1-4 binstall 自动安装改 opt-in
- 默认：发现 binstall 不可用 → 走 `cargo install` 并提示 `   Hint: install cargo-binstall for faster updates`。
- 新增 `--install-binstall` / `--with-binstall` 显式启用自动安装。

### P1-5 长尾包可视化
- spinner message 加 elapsed seconds；超过 30s demote 出一行 `   Slow ripgrep building for 35s ...`。

### P1-6 sparse index HTTP 加 1 次快速重试 + 指数退避
- 限定到 1 次重试（500ms），不引入 `reqwest_retry`。

### P1-7 `parse_source` 兜底太松
- 新增 `PackageSource::Unknown(String)`；`check_package_updates` 跳过，UI 显示 `[unknown source]`。

### P1-8 `INSTALLED_VERSION_CACHE.set()` 静默失败
- 改 `OnceLock<Mutex<HashMap>>` 初始化用 `get_or_init`，写入用 `lock().clear() + extend`，保留 `invalidate_*` 语义。

### P1-9 集成测试 + CLI 快照
- `tests/cli.rs`：`assert_cmd` + `insta` 快照 `cargo-fresh --help`、`completion bash` 输出。
- `tests/sparse_index_offline.rs`：`wiremock` 模拟 200/404/超时 三种响应。

### P1-10 错误模型：thiserror 提取可执行错误
- 定义 `enum CargoFreshError { ProxyBlocked, RegistryUnreachable, BinstallMissing, ... }`。
- `main` 末端 match 给出可执行建议。其他保留 `anyhow`。

### P1-11 1.0 文档套件
- 补 `.github/ISSUE_TEMPLATE/`、`.github/PULL_REQUEST_TEMPLATE.md`、`CONTRIBUTING.md`、`SECURITY.md`。
- README 加 "Stability Guarantees"、"How cargo-fresh differs from cargo-update" 两节。

## P2 — 1.x evolution

1. **P2-1** `--locked` / `--frozen` 透传给 `cargo install`。
2. **P2-2** `--include-package <name>` 可重复，与 `--exclude` 对称。
3. **P2-3** `cargo fresh outdated` 子命令（只查不升）。
4. **P2-4** 配置文件 `~/.config/cargo-fresh/config.toml`。
5. **P2-5** Windows + Linux aarch64 release matrix。
6. **P2-6** `cargo fresh self update`（不做 self-replace，交给 cargo）。
7. **P2-7** `tracing` + `--log-level` / `RUST_LOG`，verbose 的 `println!` 走 `tracing::debug!`。
8. **P2-8** dialoguer 多选区分 stable / prerelease group，UI 暗示"当前是预发布 → 可升预发布"。

## "Modern Rust CLI" gaps to close

1. `CommandFactory` 派生统一 CLI（→ P0-1）。
2. `is_terminal` + `supports-color`，非 TTY 自动降级。
3. `clap` 的 `#[command(verbatim_doc_comment, long_about = ...)]` —— `--help` 解释和 `cargo install --force` 的区别。
4. `concolor` / `anstream` 替代 `colored`，且 status_err 走 stderr 而非 stdout。
5. `miette` / `color-eyre` 给顶层错误打印带 source chain 的视图。
6. `tokio` features 精确化（→ P1-1）。
7. `cargo-deny` + `cargo-audit` 加进 CI。
8. `cargo-dist` 替代手写 release matrix。
9. `assert_cmd` + `insta` 做 CLI 快照（→ P1-9）。
10. `etcetera` / `xdg` 处理配置目录（→ P2-4）。

## Differentiation vs `cargo-update`

1. sparse index 默认 + 自带兜底（README 第一段）。
2. binstall 一等公民（具体的 N 包 / 秒 对比）。
3. glob 默认模糊匹配（`build_globset` 自动 `*pattern*`）。
4. 源感知升级策略（git / path 也能升）。
5. CI 友好：`--format=json` + 稳定退出码契约（→ P0-5）。
