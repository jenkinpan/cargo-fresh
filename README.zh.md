# cargo-fresh

[![Crates.io](https://img.shields.io/crates/v/cargo-fresh.svg)](https://crates.io/crates/cargo-fresh)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Wiki](https://img.shields.io/badge/wiki-Recipes_·_FAQ_·_Troubleshooting-blue)](https://github.com/jenkinpan/cargo-fresh/wiki)


<div align="center">

**Language / 语言**

[![English](https://img.shields.io/badge/English-英文版-blue?style=for-the-badge)](README.md) [![中文](https://img.shields.io/badge/中文-Current-green?style=for-the-badge)](README.zh.md)

</div>

---

> 📣 **1.0 临近**。正在收集真实使用反馈，用来锁定 1.0 契约（CLI 形态、`--format=json` schema、退出码、错误提示）。反馈窗口截至 **2026-06-30**，之后切 `1.0.0-rc.1`。请在 [#3 Towards 1.0 — Feedback Wanted](https://github.com/jenkinpan/cargo-fresh/issues/3) 留言。

---

一个用 Rust 编写的工具，用于检查和管理全局安装的 Cargo 包更新。支持交互式更新、智能预发布版本检测和彩色输出。安装后可以通过 `cargo fresh` 命令使用。工具会自动检测系统语言并相应显示中文或英文界面。

## 功能特性

- 🔍 自动检测已安装的全局 Cargo 包
- 📦 检查每个包的最新版本
- 🎨 彩色输出，清晰显示更新状态
- ⚡ **并发处理**，快速检查多个包（3-5倍速度提升）
- 🛠️ 命令行参数支持，灵活使用
- 🔄 默认交互式更新模式，一键更新包
- 🧠 智能预发布版本检测和询问
- 🌍 自动语言检测（中文/英文）
- 🚀 Cargo 子命令支持（`cargo fresh`）
- 🌐 双语界面，智能语言切换
- 🚀 **批量操作** - 自动更新所有包，无需确认
- 🔍 **包过滤** - 用 `--filter` 保留匹配的包，用 `--exclude`（可重复）剔除，均支持通配符
- 🧪 **预演模式** - 用 `--dry-run` 预览将要执行的 cargo 命令，不做任何改动
- 📦 **来源感知更新** - 支持 crates.io、`git`（`--git URL [--rev]`）和本地 `path` 安装，带 `[git]` / `[path]` 标记
- 🛡️ **增强错误处理** - 智能重试机制和用户友好的错误消息
- 📊 **快速版本检查** - 直连 crates.io sparse index，连接池 + 并发限流（失败时回退 `cargo search`）
- ⚡ **快速安装** - 进程内二进制下载器直接流式拉取 GitHub Release tarball，可用时校验 sha256，原子化安装——无需 `cargo binstall` 子进程。非 GitHub 或不支持的包回退到原生 `cargo install`
- 🚀 **并发更新** - `-j N` / `--jobs N` 同时跑最多 N 个包的更新（默认 4，`0` 表示无限制，`1` 退回串行）。rustup 风格的堆叠进度行
- 🐙 **GitHub Releases API** - probe + fetch 用 `GET /repos/{owner}/{repo}/releases/tags/{tag}` 一次性拿到匹配的 asset（API 不可达时自动回退 HEAD probe）。设置 `GITHUB_TOKEN` / `GH_TOKEN`，或 `gh auth login` 已配置时,配额从 60/hr 提升到 5000/hr

## 安装

### 从 crates.io 安装（推荐）

```bash
cargo install cargo-fresh
```
or
```bash
# 使用预编译二进制文件进行快速安装
cargo binstall cargo-fresh
```

**注意**: cargo-fresh 通过 GitHub Releases API 直接拉取 GitHub Release 二进制（API 不可达时自动回退到 HEAD probe），并在进程内流式下载——不会调用、不依赖、也不会自动安装 `cargo binstall`。GitHub API 匿名配额是 60 次/小时;设置 `GITHUB_TOKEN`（或 `GH_TOKEN`，或已配置 `gh auth login`）可提升到 5000 次/小时——主要在 `--check-prebuilt` 批量探测多个包时才会触及配额。

### 从源码安装

```bash
# 克隆项目
git clone https://github.com/jenkinpan/cargo-fresh.git
cd cargo-fresh

# 构建并安装
cargo install --path .
```

### 从 GitHub 安装

```bash
cargo install --git https://github.com/jenkinpan/cargo-fresh.git
```

## 语言支持

工具会自动检测您的系统语言并相应显示界面：

- **中文环境**：自动显示中文界面
- **英文环境**：自动显示英文界面
- **语言检测**：基于系统环境变量（LANG, LC_ALL, LC_CTYPE）

您也可以通过设置环境变量手动覆盖语言：

```bash
# 强制英文界面
LANG=en_US.UTF-8 cargo fresh

# 强制中文界面
LANG=zh_CN.UTF-8 cargo fresh
```

## 使用方法

### 基本使用

安装后，您可以通过以下两种方式使用：

```bash
# 方式1：作为 cargo 子命令（推荐）
cargo fresh

# 方式2：直接调用
cargo-fresh
```

### 命令行选项

- `-v, --verbose`: 显示详细信息
- `-u, --updates-only`: 只显示有更新的包
- `--no-interactive`: 非交互模式（默认是交互模式）
- `--include-prerelease`: 包含预发布版本（alpha、beta、rc 等）
- `--batch`: 批量模式 - 自动更新所有包，无需确认
- `--filter <模式>`: 仅保留匹配该通配符模式的包（`*`、`?`、`[abc]`）
- `--exclude <模式>`: 剔除匹配该通配符模式的包；可重复，在 `--filter` 之后应用
- `--dry-run`: 打印将要执行的 cargo 命令但不实际执行
- `--registry-url <URL>`: 覆盖 sparse index 的 base URL（镜像支持）
- `--format <FORMAT>`: `human`（默认）或 `json`（供 CI 消费）
- `--check-prebuilt`: 在检查阶段用 cargo-fresh 自己的下载器（GitHub Releases API + HEAD probe 兜底）探测每个候选包，标记是否有预编译产物（`[prebuilt]`）/ 会回退到源码编译（`[source]`）/ 结果不确定（`[unknown]`）。默认关闭——每次探测会发起若干 HTTP 请求；API 配额健康时通常亚秒级。不再 spawn `cargo` 子进程，也不需要 `cargo-binstall`
- `-j, --jobs <N>`: 并发更新的包数上限。默认 `4`；`0` 表示无限制（每个选中包一个任务）；`1` 退回 0.11.x 串行行为。`cargo install` 回退路径会在 cargo 的 `$CARGO_HOME` 锁上自然排队，不受此参数影响
- `-h, --help`: 显示帮助信息
- `-V, --version`: 显示版本信息

### 示例

```bash
# 检查所有包并显示详细信息
cargo fresh --verbose

# 只显示有更新的包
cargo fresh --updates-only

# 组合使用
cargo fresh --verbose --updates-only

# 默认交互模式（推荐）
cargo fresh

# 只显示有更新的包（交互模式）
cargo fresh --updates-only

# 非交互模式
cargo fresh --no-interactive

# 包含预发布版本检查（交互模式）
cargo fresh --include-prerelease

# 非交互模式 + 预发布版本
cargo fresh --no-interactive --include-prerelease

# 批量模式 - 自动更新所有包，无需确认
cargo fresh --batch

# 按名称模式过滤包（支持通配符模式）
cargo fresh --filter "cargo*"              # 只检查以 "cargo" 开头的包
cargo fresh --filter "*mdbook*"            # 只检查包含 "mdbook" 的包
cargo fresh --filter "nu*"                 # 只检查以 "nu" 开头的包

# 按通配符模式排除包（可重复，在 --filter 之后应用）
cargo fresh --exclude "cargo-fresh"                  # 检查除 cargo-fresh 外的所有包
cargo fresh --exclude "ripgrep" --exclude "tokei"    # 跳过多个包
cargo fresh --filter "cargo*" --exclude "cargo-fresh"  # cargo* 包，但排除 cargo-fresh

# 预演：预览将要执行的 cargo 命令，不做任何改动
cargo fresh --dry-run                      # 预览所有包的更新
cargo fresh --dry-run --batch              # 预览完整的批量更新

# 组合新选项与现有选项
cargo fresh --batch --filter "cargo*"      # 批量更新只更新 cargo 包
cargo fresh --verbose --filter "*mdbook*"  # 详细检查 mdbook 包
cargo fresh --batch --updates-only        # 批量更新只更新有更新的包

# 生成 shell 补全脚本
cargo fresh completion zsh    # 生成 zsh 补全
cargo fresh completion bash   # 生成 bash 补全
cargo fresh completion fish   # 生成 fish 补全

# 生成 cargo fresh 子命令补全
cargo fresh completion zsh --cargo-fresh    # 生成 cargo fresh zsh 补全
cargo fresh completion bash --cargo-fresh   # 生成 cargo fresh bash 补全
```

### Shell 补全安装

#### Zsh
```bash
# 生成并安装 zsh 补全
cargo-fresh completion zsh > ~/.zsh/completions/_cargo-fresh
# 或者为 cargo fresh 子命令
cargo-fresh completion zsh --cargo-fresh > ~/.zsh/completions/_cargo

# 添加到你的 ~/.zshrc
echo 'fpath=(~/.zsh/completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc
```

#### Bash
```bash
# 生成并安装 bash 补全
cargo-fresh completion bash > ~/.local/share/bash-completion/completions/cargo-fresh
# 或者为 cargo fresh 子命令
cargo-fresh completion bash --cargo-fresh > ~/.local/share/bash-completion/completions/cargo

# 在你的 ~/.bashrc 中加载
echo 'source ~/.local/share/bash-completion/completions/cargo-fresh' >> ~/.bashrc
```

#### Fish
```bash
# 生成并安装 fish 补全
cargo-fresh completion fish > ~/.config/fish/completions/cargo-fresh.fish
# 或者为 cargo fresh 子命令
cargo-fresh completion fish --cargo-fresh > ~/.config/fish/completions/cargo.fish
```

#### Nushell
```bash
# 生成并安装 nushell 补全
cargo-fresh completion nushell > ~/.config/nushell/completions/cargo-fresh.nu
# 或者为 cargo fresh 子命令
cargo-fresh completion nushell --cargo-fresh > ~/.config/nushell/completions/cargo.nu
```

## 输出示例

cargo-fresh 采用 cargo 风格的状态格式：12 字符右对齐的加粗动词 + 内容。
颜色承载语义——绿色（成功）、黄色（警告）、红色（失败）、灰色（次要信息）。
全程无 emoji。

### 交互模式（默认）

```text
    Checking 全局 cargo 包更新
       Found 5 个已安装的包
       Fresh ripgrep 14.1.1
    Updating cargo-outdated 0.16.0 -> 0.17.0
    Updating devtool 0.2.4 -> 0.2.5

可用更新：
稳定版更新：
    Updating cargo-outdated 0.16.0 -> 0.17.0
    Updating devtool 0.2.4 -> 0.2.5
预发布版更新：
  Prerelease mdbook 0.4.52 -> 0.5.0-alpha.1

更新这些包？ y
是否包含预发布版？ n
选择要更新的包（空格切换，回车确认）
> [x] cargo-outdated
> [x] devtool

    Updating 选中的包
    cargo-outdated  resolving
    cargo-outdated  [#####################>                ] 1.2 MiB/2.1 MiB
    cargo-outdated  installed 2.10 MiB
          devtool  resolving
          devtool  installed 1.42 MiB

更新摘要
       预编译 cargo-outdated, devtool
    Finished 2 个成功, 耗时 4.2s
```

每一行是 `MultiProgress` 持有的实时进度行：
`pending` → `resolving` → `downloading X.X MiB`（或已知 `Content-Length` 时显示字节进度条）→ `installed X.XX MiB`，
完成后定格成静态行,屏幕上累积保留完整历史。带 `-j N`（默认 4）时多个行并发推进；最终的摘要按输入顺序列出每个包,与完成顺序无关。

### 预演模式

`--dry-run` 打印将要执行的 cargo 命令但不做任何改动：

```text
    Checking 全局 cargo 包更新
       Found 5 个已安装的包
    Updating cargo-outdated 0.16.0 -> 0.17.0

     Dry run 不会实际修改任何包
   Would run cargo-outdated: cargo install --force cargo-outdated --version 0.17.0
```

### 非交互模式

`--no-interactive` 模式只列出可用更新，不更新任何包（用 `--batch` 自动更新）：

```text
    Checking 全局 cargo 包更新
       Found 5 个已安装的包
       Fresh ripgrep 14.1.1
    Updating mdbook 0.4.52 -> 0.5.0-alpha.1
       Note 未选择任何包
```

git 和 path 安装会带一个灰色的 `[git]` / `[path]` 标记，例如
`    Updating my-tool 0.1.0 -> 0.2.0 [git]`。

## Shell 补全支持

`cargo-fresh` 支持多种 shell 的自动补全功能，让命令行使用更加便捷。

### 支持的 Shell

- **Zsh** - 完整的补全支持
- **Bash** - 基础补全支持
- **Fish** - 原生补全支持
- **PowerShell** - Windows 补全支持
- **Elvish** - 现代 shell 补全支持
- **Nushell** - Nushell 补全支持

### 安装补全

#### 手动安装

```bash
# 1. 生成补全脚本
cargo fresh completion zsh > ~/.zsh_completions/cargo-fresh.zsh

# 2. 添加到 zsh 配置
echo 'fpath=($HOME/.zsh_completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc

# 3. 重新加载配置
source ~/.zshrc
```

#### Cargo Fresh 子命令补全

为 `cargo fresh` 子命令生成补全：

```bash
# 生成 cargo fresh 子命令补全
cargo fresh completion zsh --cargo-fresh > cargo-fresh-completion.zsh
cargo fresh completion bash --cargo-fresh > cargo-fresh-completion.bash

# 安装 cargo fresh 补全
source cargo-fresh-completion.zsh  # For zsh
source cargo-fresh-completion.bash # For bash
```

#### 其他 Shell 安装

```bash
# Bash 补全
cargo fresh completion bash > ~/.bash_completions/cargo-fresh.bash
echo 'source ~/.bash_completions/cargo-fresh.bash' >> ~/.bashrc

# Fish 补全
cargo fresh completion fish > ~/.config/fish/completions/cargo-fresh.fish

# PowerShell 补全
cargo fresh completion powershell > cargo-fresh.ps1
```

### 使用方法

安装完成后，您可以通过两种方式使用自动补全：

#### 直接命令补全
```bash
cargo fresh <TAB>
# 显示所有可用选项：
# --batch  --dry-run  --exclude  --filter  --help  --include-prerelease
# --no-interactive  --updates-only  --verbose  --version
```

#### Cargo 子命令补全
```bash
cargo <TAB>        # 显示 'fresh' 作为子命令
cargo fresh <TAB>  # 显示所有 fresh 选项和参数
```

## 技术特性

- **Sparse Index 检查**: 直接通过 HTTP 查询 crates.io sparse index（共享连接池、并发限流），不再为每个包启动 `cargo search`；仅在索引不可达时回退到 `cargo search`
- **并发处理**: 使用 Tokio 异步运行时并发检查包
- **基于 semver 的比较**: 使用真正的 semver 排序，被 yank 的版本回滚不会误报为更新，而 `1.0.0+build` 重新发布会
- **来源感知更新**: 检测 crates.io / git / path 安装来源，为每种来源选择正确的 `cargo install` 策略
- **智能版本检测**: 自动区分稳定版本和预发布版本
- **交互式界面**: 用户友好的命令行交互体验
- **彩色输出**: 美观的终端输出，清晰的状态显示
- **增强错误处理**: 智能重试机制，指数退避和用户友好的错误消息
- **批量操作**: 支持自动化批量更新，无需用户确认
- **包过滤**: 高级过滤功能，支持通配符模式
- **类型安全**: Rust 类型系统保证代码安全性
- **进度条**: 实时显示更新进度，提升用户体验
- **Shell 补全**: 支持多种 shell 的自动补全功能
- **语言检测**: 自动系统语言检测和界面适配
- **Cargo 集成**: 原生 cargo 子命令支持，无缝工作流
- **双语支持**: 完整的中英文界面，智能切换
- **模块化架构**: 清晰、可维护的代码结构，分离模块

## Shell 补全故障排除

### 常见问题

#### 补全不工作
如果 shell 补全不工作，请尝试以下步骤：

1. **验证补全安装**：
   ```bash
   # 检查补全文件是否存在
   ls ~/.zsh/completions/_cargo-fresh  # 对于 zsh
   ls ~/.local/share/bash-completion/completions/cargo-fresh  # 对于 bash
   ```

2. **重新加载 shell 配置**：
   ```bash
   # 对于 zsh
   source ~/.zshrc
   
   # 对于 bash
   source ~/.bashrc
   
   # 对于 fish
   # 重启 fish shell
   ```

3. **重新生成补全文件**：
   ```bash
   # 生成新的补全文件
   cargo-fresh completion zsh > ~/.zsh/completions/_cargo-fresh
   cargo-fresh completion bash > ~/.local/share/bash-completion/completions/cargo-fresh
   ```

#### 补全中缺少选项
如果你注意到补全中缺少某些选项：

1. **更新 cargo-fresh**：
   ```bash
   cargo install --force cargo-fresh
   ```

2. **重新生成补全文件**：
   ```bash
   cargo-fresh completion zsh > ~/.zsh/completions/_cargo-fresh
   ```

3. **验证补全包含新选项**：
   ```bash
   grep -E "(batch|filter)" ~/.zsh/completions/_cargo-fresh
   ```

#### Cargo fresh 子命令补全
对于 `cargo fresh` 子命令补全：

1. **生成 cargo fresh 补全**：
   ```bash
   cargo-fresh completion zsh --cargo-fresh > ~/.zsh/completions/_cargo
   ```

2. **验证 cargo 补全**：
   ```bash
   cargo <TAB>  # 应该显示 'fresh' 作为子命令
   cargo fresh <TAB>  # 应该显示所有 fresh 选项
   ```

## 贡献

欢迎贡献代码！请遵循以下步骤：

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

本项目采用 Apache 2.0 许可证。查看 [LICENSE](LICENSE) 文件了解完整的许可证条款。

### 许可证摘要

Apache 2.0 许可证是一个宽松的开源许可证，允许您：

- ✅ **商业使用** - 在商业项目中使用
- ✅ **修改** - 修改源代码
- ✅ **分发** - 分发原始或修改后的代码
- ✅ **私人使用** - 私人使用
- ✅ **专利使用** - 使用相关专利
- ✅ **专利授权** - 自动授予专利许可

**主要要求**：
- 在分发时必须包含原始许可证和版权声明
- 必须说明对源代码的修改
- 不能使用项目名称、商标或产品名称进行推广

### 版权信息

Copyright (c) 2025 Jenkin Pan

本项目基于 Apache 2.0 许可证开源，详见 [LICENSE](LICENSE) 文件。

## 相关链接

- [Crates.io](https://crates.io/crates/cargo-fresh)
- [GitHub Repository](https://github.com/jenkinpan/cargo-fresh)
- [Issues](https://github.com/jenkinpan/cargo-fresh/issues)
