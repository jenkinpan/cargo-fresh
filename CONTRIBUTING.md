# Contributing to cargo-fresh

感谢愿意贡献！本文档记录最关键的几条约定，超出范围的细节看 `CLAUDE.md` 与 `ROADMAP.md`。

## 开发环境

- **MSRV**: Rust 1.82。本地用最新 stable 也行；CI 会同时跑 stable 与 1.82 两个 job
- **依赖管理**: `Cargo.lock` 入库（这是 binary crate，要求可复现构建）

## 提交前检查清单

```bash
cargo clippy --all-targets -- -D warnings   # 零警告策略
cargo test                                  # 单元 + 集成测试都要绿
cargo build --release                       # release 编译路径偶尔会暴露 debug 漏掉的警告
```

如果改了用户可见的行为，还要更新：

- `CHANGELOG.md` 的 `[Unreleased]` 节
- `README.md`（中英文 README 同时更新）
- `src/locale/texts.rs`（如有新文案，必须英中两份齐全 + `tests::test_text_consistency` 列表登记）

## 代码风格

### CLI 输出

**`println!` 一律走 `display::status*` 家族**——cargo 风格的 12 字符右对齐动词头是项目的视觉签名，详见 `CLAUDE.md` 的「Status verb dictionary」。新增动词请加到那张表里。

颜色含义：

| 颜色 | 用途 |
|------|------|
| 绿色 bold | 成功类（Checking / Found / Updated / Finished） |
| 黄色 bold | 警告类（Fallback / Note / Unchanged / Skip） |
| 红色 bold | 失败类（Failed / Finished 带失败时） |
| 灰色 dim | 次要信息（Fresh / Package N/M / Hint / Check / Latest） |

### 错误处理

- 内部路径继续用 `anyhow::Result`，错误链短不要硬塞 thiserror
- 用户**可执行**的失败模式（"装下 X"、"设个 HTTPS_PROXY"）才适合下沉到 `src/errors.rs` 的 `CargoFreshError`
- `main` 通过 `errors::hint_for` 在错误链里嗅探，给出具体提示行

### 国际化

- 全部用户可见字符串在 `src/locale/texts.rs`
- 多占位符模板**必须**用命名占位符 + `language.format_text("key", &[("name", val)])`；老式 `.replace("{}", x)` 链式调用只能在单占位符场景使用
- 加新语言：补一个 `match` 分支 + 更新 `texts::tests::test_text_consistency` 的键列表

## 测试

- 单元测试与代码同文件 `#[cfg(test)] mod tests`
- 集成测试在 `tests/`：
  - `tests/cli.rs` 用 `assert_cmd` 跑真实二进制的对外契约（不做 byte-for-byte 快照，避免改个标点炸一片）
  - `tests/sparse_index_http.rs` 用 `wiremock` 离线验证 HTTP 行为
- locale 检测：用 `detect_from_locale(&str)` 纯函数；**绝不** `env::set_var`

## Commit & PR

- Commit 标题用 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/v1.0.0/) 风格：`feat(scope): ...` / `fix(scope): ...` / `refactor(scope): ...` / `test: ...` / `docs: ...`
- BREAKING change 标 `!`，例：`feat(binstall)!: 默认不再自动安装 cargo-binstall`
- 描述用中文或英文均可；项目内既有提交两种都有

## 发布流程

完全自动化，详见 `CLAUDE.md` 的「Release process」节。短版：

```bash
# 1. bump 版本
$EDITOR Cargo.toml CHANGELOG.md
git commit -am "chore: release vX.Y.Z"

# 2. 打 tag 并推送
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin master vX.Y.Z
```

剩下的 crates.io 发布 + 多平台二进制构建由 GitHub Actions 接管。

## 1.0 之前的稳定性

参见 README 的「Stability Guarantees」节。简言之：CLI 退出码 / `--format=json` 的 `schema_version=1` 字段是承诺的；颜色与中英文字面文案随时可能微调。

## 行为准则

参与 issue、PR、Discussions 时请保持友善与建设性。攻击性发言会被 close + lock。

## License

提交即同意按 Apache-2.0 发布。
