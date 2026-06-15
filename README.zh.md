# cargo-fresh

[![Crates.io](https://img.shields.io/crates/v/cargo-fresh.svg)](https://crates.io/crates/cargo-fresh)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Wiki](https://img.shields.io/badge/wiki-Recipes_·_FAQ_·_Troubleshooting-blue)](https://github.com/jenkinpan/cargo-fresh/wiki)

<div align="center">

**Language / 语言**

[![English](https://img.shields.io/badge/English-英文版-blue?style=for-the-badge)](README.md) [![中文](https://img.shields.io/badge/中文-Current-green?style=for-the-badge)](README.zh.md)

</div>

---

> **1.0 临近**。1.0 契约（CLI 形态、`--format=json` schema、退出码、错误提示）的反馈窗口截至 **2026-06-30**，之后切 `1.0.0-rc.1`。请在 [#3 Towards 1.0 — Feedback Wanted](https://github.com/jenkinpan/cargo-fresh/issues/3) 留言。

---

`cargo-fresh` 用于检查和更新全局安装的 Cargo 包。它并发查询 crates.io sparse index、优先使用 GitHub Release 的预编译二进制（必要时回退到 `cargo install`）、用 `-j N` 并发更新，并提供稳定的 `--format=json` 契约供脚本消费。通过 `cargo install cargo-fresh` 安装，使用 `cargo fresh` 调用。

## 目录

- [亮点](#亮点)
- [安装](#安装)
- [快速开始](#快速开始)
- [CLI 参考](#cli-参考)
- [退出码](#退出码)
- [JSON 输出](#json-输出)
- [Shell 补全](#shell-补全)
- [输出示例](#输出示例)
- [语言检测](#语言检测)
- [稳定性承诺](#稳定性承诺)
- [1.0 契约](#10-契约)
- [与 cargo-update 的区别](#与-cargo-update-的区别)
- [贡献](#贡献)
- [许可证](#许可证)

## 亮点

- **快速版本检查** —— 直接走 crates.io sparse index（HTTP，每包约 50–100 ms），共享连接池 + 16 路并发上限。仅在 sparse index 不可达时回退 `cargo search`。
- **来源感知更新** —— crates.io、`git+URL [--rev]`、`path+DIR` 各自使用正确的 `cargo install` 策略；输出带 `[git]` / `[path]` 标记。
- **进程内二进制下载器** —— 通过 GitHub Releases API（不可达时 HEAD probe 回退）直接拉取 Release tarball，存在 `.sha256` 边车时校验，原子安装到 `~/.cargo/bin`。**不**调用 `cargo binstall`。
- **并发更新** —— `-j N` / `--jobs N`（默认 4）以 rustup 风格的堆叠进度行并发更新；`-j 1` 退回完全串行。
- **过滤** —— `--filter PATTERN` 保留匹配，`--exclude PATTERN`（可重复）剔除；均支持通配符（`*`、`?`、`[abc]`）。
- **`--dry-run`** 仅打印将要执行的 `cargo install …` 命令，不做任何改动。
- **`--format=json`** 在 stdout 输出一行机器可读 JSON（Draft 2020-12 schema，`schema_version=2`），并禁用所有 spinner / 提示。契约稳定；大版本内仅做加性变更。
- **保留安装选项** —— 从 `.crates2.json` 读取 `--features` / `--no-default-features` / `--all-features` 并在更新时回填。
- **双语界面** —— 通过 `LANG` / `LC_ALL` / `LC_CTYPE` 自动切换中英文。

## 安装

### 从 crates.io 安装（推荐）

```bash
cargo install cargo-fresh
# 或者，如果已经有 cargo-binstall：
cargo binstall cargo-fresh
```

cargo-fresh 通过 GitHub Releases API（不可达时回退 HEAD probe）直接拉取 GitHub Release 二进制，**不**调用、**不**依赖、**也不会**自动安装 `cargo binstall`。GitHub API 匿名配额是 60 次/小时；设置 `GITHUB_TOKEN`（或 `GH_TOKEN`，或已配置 `gh auth login`）可提升到 5 000 次/小时——主要在 `--check-prebuilt` 批量探测多个包时才会触及配额。

### 从源码安装

```bash
git clone https://github.com/jenkinpan/cargo-fresh.git
cd cargo-fresh
cargo install --path .
```

### 直接从 GitHub 安装

```bash
cargo install --git https://github.com/jenkinpan/cargo-fresh.git
```

## 快速开始

```bash
# 交互模式：列出可更新包，选要装的
cargo fresh

# 无提示直接全部更新
cargo fresh --batch

# 预览将执行的 cargo 命令，不改动任何东西
cargo fresh --dry-run --batch

# 只更新匹配的包
cargo fresh --batch --filter "cargo-*"

# CI 检查：有更新即 exit 1
cargo fresh --format=json
```

## CLI 参考

| 参数 | 说明 |
|------|------|
| `-v, --verbose` | 显示每个包的检查细节 |
| `-u, --updates-only` | 只列出有更新的包 |
| `--no-interactive` | 跳过交互提示；仅列出更新但不安装（要安装请加 `--batch`） |
| `--batch` | 无提示直接应用所有选中的更新 |
| `--include-prerelease` | 把 `α / β / rc` 版本视为候选 |
| `--filter <PATTERN>` | 只保留匹配 glob 的包（`*`、`?`、`[abc]`） |
| `--exclude <PATTERN>` | 剔除匹配的包；可重复；在 `--filter` 之后应用 |
| `--dry-run` | 打印将要执行的 `cargo install …` 命令而不真正运行 |
| `--registry-url <URL>` | 覆盖 sparse-index 基础 URL（镜像支持） |
| `--no-cargo-search-fallback` | sparse index 失败时不回退 `cargo search`（等价 `CARGO_FRESH_NO_FALLBACK=1`） |
| `--check-prebuilt` | 探测每个候选包，标记 `[prebuilt]` / `[source]` / `[unknown]`。默认关——每包会发几个 HEAD 请求 |
| `--debug` | 向 stderr 输出 downloader 决策 trace，供 issue 排查使用。不属于 1.0 稳定契约；不要解析它 |
| `-j, --jobs <N>` | 并发更新数。默认 `4`；`0` = 不限；`1` = 串行。`cargo install` 回退路径会在 cargo 的 `$CARGO_HOME` 锁上自然串行化 |
| `--format <FORMAT>` | `human`（默认）或 `json` |
| `-h, --help` / `-V, --version` | 帮助 / 版本 |

子命令：`cargo fresh completion <shell> [--install] [--yes]`（见 [Shell 补全](#shell-补全)）以及 `cargo fresh man`（stdout 是 TTY 时调系统 `man`，否则输出 raw roff）。

## 退出码

自 0.10.0 起的稳定契约：

| 码值 | 含义 |
|------|------|
| 0    | 没有可用更新，或选中的更新全部成功 |
| 1    | 有可用更新但未应用（`--format=json` 未配合 `--batch`，或 `--no-interactive` 且没有选中任何包） |
| 2    | 至少一个更新失败 |
| 130  | 用户按下 Ctrl-C；剩余包跳过 |

```bash
# 任意全局包有更新即让 CI 失败
cargo fresh --format=json
# → 有更新则 exit 1，否则 0

# CI 全量更新，任一失败即非零
cargo fresh --format=json --batch
# → 任一失败则 exit 2，否则 0
```

## JSON 输出

`--format=json` 在 **stdout** 输出一个 JSON 对象，所有状态行 / 错误 / 提示走 **stderr**。因此 `cargo fresh --format=json | jq '.'` 可直接消费，`cargo fresh > /dev/null` 仍能在终端看到进度。

完整 schema 见 [`docs/json-schema.json`](docs/json-schema.json)（JSON Schema Draft 2020-12）。`schema_version=2` 是 1.0 之前最后一次破坏性 schema 变更（重命名了 `updates_available[].binstall` → `prebuilt`，枚举 `source_build` → `source`）；在 `schema_version=2` 内字段只增不改。

在原始 `1` 形态之上 `schema_version=2` 已加入的字段：

- **`skipped[].reason_code`** —— 稳定枚举（`path_source` / `git_source` / `unknown_source`）。脚本判断请用这个而非 `reason` 字符串。
- **`version_check_errors[]`** —— 版本查询失败的包，含 `name`、`kind`（`not_found` / `unavailable`）、可读 `error`。这些包不会出现在 `updates_available[]` 里。
- **`summary.selected` / `attempted` / `check_errors`** —— 已选 / 已尝试安装 / 查询失败的包数。
- **`version`**（顶层）—— 产出这份报告的 cargo-fresh 版本（如 `"0.12.5"`），让归档的 JSON 自描述。脚本判断请用 `schema_version` / `format`，不要用它。
- **`results[].install_method`** —— 实际走的安装路径：`prebuilt`（downloader 拉到预编译二进制）/ `source`（回退到 `cargo install`）/ `null`（安装未完成）。与 `updates_available[].prebuilt` 共用词汇表，可对比 `--check-prebuilt` 的预测与实际结果。

```bash
# 列出所有可更新包名
cargo fresh --format=json | jq -r '.updates_available[].name'

# 批量更新后的失败计数
cargo fresh --format=json --batch | jq '.summary.failed'

# 仅 git 源的候选
cargo fresh --format=json | jq '.updates_available[] | select(.source == "git")'

# 检测是否被 Ctrl-C 打断
cargo fresh --format=json --batch | jq '.aborted'

# 列出版本查询失败的包（网络抖动等）
cargo fresh --format=json | jq '.version_check_errors[]'

# 用稳定枚举判断跳过原因
cargo fresh --format=json | jq '.skipped[] | select(.reason_code == "git_source")'
```

## Shell 补全

支持的 shell：**bash**、**zsh**、**fish**、**powershell**、**elvish**、**nushell**。

### 推荐：交互式安装

```bash
cargo fresh completion <shell> --install
```

`--install` 会弹出一个 MultiSelect 选择器（空格切换，回车确认）：

```
选择要安装的补全（空格切换，回车确认）
> [x] cargo-fresh<TAB>  —— 顶层二进制补全
  [x] cargo fresh<TAB>  —— cargo 子命令补全
```

两项默认全选。`cargo-fresh<TAB>` 让独立二进制可补全；`cargo fresh<TAB>` 让 cargo 子命令形式可补全。大多数用户两个都需要。

加 `--yes` 可跳过提示直接装两个（适合脚本 / CI）：

```bash
cargo fresh completion fish --install --yes
```

若目标文件已存在会逐个询问是否覆盖。对于默认目录不在自动加载路径上的 shell（zsh / powershell / elvish / nushell），cargo-fresh 会用一行 `Hint` 给出确切要在 `~/.zshrc` / `$PROFILE` / `rc.elv` / `config.nu` 里加的那行配置。

### 安装路径

| Shell | 顶层（`cargo-fresh<TAB>`） | cargo 子命令（`cargo fresh<TAB>`） |
|-------|----------------------------|------------------------------------|
| bash       | `~/.local/share/bash-completion/completions/cargo-fresh` | `~/.local/share/bash-completion/completions/cargo` |
| zsh        | `~/.zfunc/_cargo-fresh`（需把 `~/.zfunc` 加入 `$fpath`） | `~/.zfunc/_cargo` |
| fish       | `~/.config/fish/completions/cargo-fresh.fish` | `~/.config/fish/completions/cargo.fish` |
| nushell    | `~/.config/nushell/completions/cargo-fresh.nu` | `~/.config/nushell/completions/cargo.nu` |
| elvish     | `~/.config/elvish/lib/cargo-fresh.elv` | `~/.config/elvish/lib/cargo.elv` |
| powershell | `~/.config/powershell/cargo-fresh.ps1` | `~/.config/powershell/cargo.ps1` |

设置了 `XDG_CONFIG_HOME` / `XDG_DATA_HOME` 时优先使用。

### 手动安装（重定向 stdout）

如果你想自己选目录，去掉 `--install` 用重定向：

```bash
# 顶层二进制补全
cargo fresh completion zsh > ~/.zfunc/_cargo-fresh

# cargo 子命令形式
cargo fresh completion zsh --cargo-fresh > ~/.zfunc/_cargo
```

`--cargo-fresh` 用来在两份脚本之间切换。配合 `--install` 时该标志被忽略——选择器已经覆盖两个目标。

## 输出示例

cargo-fresh 使用 cargo 风格状态行：12 字符右对齐加粗动词 + 内容。颜色携带语义——绿（成功）、黄（警告）、红（失败）、暗（次要）。无 emoji。

### 交互模式（默认）

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
       Fresh ripgrep 14.1.1
    Updating cargo-outdated 0.16.0 -> 0.17.0
    Updating devtool 0.2.4 -> 0.2.5

Updates available:
Stable updates:
    Updating cargo-outdated 0.16.0 -> 0.17.0
    Updating devtool 0.2.4 -> 0.2.5

Update these packages? y
Select packages (space to toggle, enter to confirm)
> [x] cargo-outdated
> [x] devtool

  cargo-outdated  resolving
  cargo-outdated  [######################>      ] 1.2 MiB/2.1 MiB
  cargo-outdated  installed 2.10 MiB
         devtool  resolving
         devtool  installed 1.42 MiB

Update Summary
    Prebuilt cargo-outdated, devtool
    Finished 2 succeeded, in 4.2s
```

每行是 `MultiProgress` 上的一行实时状态：`pending` → `resolving` → `downloading X.X MiB`（当 `Content-Length` 已知时显示进度条）→ `installed X.XX MiB`，结束后固化为静态行，屏幕累积完整历史。配合 `-j N`（默认 4）这些行并发更新；最终摘要按选择顺序列包名，与完成顺序无关。

### 预演模式

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
    Updating cargo-outdated 0.16.0 -> 0.17.0

    Dry run no packages will be modified
   Would run cargo-outdated: cargo install --force cargo-outdated --version 0.17.0
```

### 非交互模式

`--no-interactive` 只列出可更新包但不安装（需要安装请加 `--batch`）：

```text
    Checking for updates to globally installed packages
       Found 5 installed package(s)
       Fresh ripgrep 14.1.1
    Updating mdbook 0.4.52 -> 0.5.0-alpha.1
       Note no packages selected
```

git / path 安装会带一个暗色 `[git]` / `[path]` 标记，例如 `Updating my-tool 0.1.0 -> 0.2.0 [git]`。

## 语言检测

cargo-fresh 通过 `LANG` / `LC_ALL` / `LC_CTYPE` 自动检测语言：

- `zh*` 区域 → 中文界面
- 其它 → 英文界面

按需覆盖：

```bash
LANG=en_US.UTF-8 cargo fresh   # 强制英文
LANG=zh_CN.UTF-8 cargo fresh   # 强制中文
```

## 稳定性承诺

1.0 前仍可能有破坏性变更；1.0.0 之后下表表面均**承诺**遵循 semver：

| 表面 | 稳定性 |
|------|--------|
| 退出码（`0` / `1` / `2` / `130`） | 稳定——同一大版本内不复用 / 不删除 |
| `--format=json` 输出，`schema_version=2` | 仅加性——可新增字段，但不重命名 / 不改类型 |
| `--help` 列出的 CLI 参数 | 稳定——弃用前至少留一个小版本周期警告 |
| 来源感知的安装行为（crates / git / path） | 稳定 |
| 人类可读的状态动词（`Checking`、`Updating` 等） | **不**稳定——措辞、颜色、对齐可能因 UX 而变 |
| 本地化文案（中 / 英） | **不**稳定——会有措辞微调，请不要在 `stdout` 上 grep |
| 内部模块 / 库 API（`cargo_fresh::*`） | **不**稳定——`src/lib.rs` 服务于集成测试，不是下游 API |

脚本对接请锚定退出码与 `--format=json`，绝不要锚定带颜色的状态行。

## 1.0 契约

1.0 前的契约清单见 [`docs/1.0-contract.md`](docs/1.0-contract.md)。它列出了 1.0 后受 semver 保护的表面、仍会保持弹性的表面，以及 `schema_version=2` 的 JSON 规则。

[#3 Towards 1.0 — Feedback Wanted](https://github.com/jenkinpan/cargo-fresh/issues/3) 的反馈窗口截至 **2026-06-30**。如果报告 downloader / 预编译二进制相关问题，请附上：

```bash
cargo fresh --debug --check-prebuilt 2>&1 | grep debug
```

`--debug` 只用于诊断；其输出格式不稳定，脚本不要消费。

## 与 cargo-update 的区别

[`cargo-update`](https://github.com/nabijaczleweli/cargo-update) 是这个生态里历史最久的工具。cargo-fresh 不是 fork，而是另起的实现——下面是促使我重新做的差异：

| | cargo-fresh | cargo-update |
|---|---|---|
| **版本来源** | crates.io sparse index（HTTP，每包约 50–100 ms，16 路并发） | 每个包跑一次 `cargo search` 子进程 |
| **来源感知更新** | crates / `git+URL` / `path+DIR` 各自使用正确的 install 命令 | 注册表 + git；无 `path` 源 |
| **包选择** | `--filter "tokio*"` + `--exclude "*-test"`（globset） | 必须精确包名或 `--all`（不支持 glob/子串） |
| **预发布处理** | 显式 `--include-prerelease`；用 semver `.pre` 判断 | 每包通过 `cargo-install-update-config` 单独开关 |
| **JSON 模式** | `--format=json`，`schema_version=2` 带版本 | 无 |
| **i18n** | 中英文经 `LANG` 自动切换 | 仅英文 |
| **Dry-run 预览** | 打印每包确切的 `cargo install` 命令 | 列出会更新哪些 |
| **二进制安装** | 进程内：GitHub Releases API + sha256 校验 + 原子安装 | 可用时调 `cargo binstall` 子进程 |
| **并发** | `-j N` 并发更新（默认 4）+ 16 路并发 index/HEAD probe | 串行更新 |
| **保留安装选项** | 是——从 `.crates2.json` 读取 features | 是——features/profile + 单包 `cargo-install-update-config` |
| **CI 友好** | 退出码 0/1/2/130 + JSON + 非 TTY 自动降级 | 标准退出码 |

cargo-update 更成熟。两者都会保留安装时的 features；cargo-update 额外保留 build profile，且支持每包配置。按需选用——两个都是健康项目。

## 贡献

详见 [CONTRIBUTING.md](CONTRIBUTING.md)。摘要：

1. Fork → 分支 → 提交 → PR。
2. 推送前 `cargo clippy --all-targets -- -D warnings` 和 `cargo test` 都必须绿。
3. 用户可见变更需要 `CHANGELOG.md` 的 `[Unreleased]` 条目 + 同步 README。

安全问题：见 [SECURITY.md](SECURITY.md)，请不要直接开 issue。

## 许可证

Apache 2.0 —— 见 [LICENSE](LICENSE)。版权所有 (c) 2025 Jenkin Pan。

## 相关链接

- [Crates.io](https://crates.io/crates/cargo-fresh)
- [GitHub 仓库](https://github.com/jenkinpan/cargo-fresh)
- [Issues](https://github.com/jenkinpan/cargo-fresh/issues)
- [Wiki](https://github.com/jenkinpan/cargo-fresh/wiki) —— 食谱、FAQ、故障排查
