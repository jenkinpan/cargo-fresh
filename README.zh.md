# cargo-fresh

[![Crates.io](https://img.shields.io/crates/v/cargo-fresh.svg)](https://crates.io/crates/cargo-fresh)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


<div align="center">

**Language / 语言**

[![English](https://img.shields.io/badge/English-英文版-blue?style=for-the-badge)](README.md) [![中文](https://img.shields.io/badge/中文-Current-green?style=for-the-badge)](README.zh.md)

</div>

---

一个用 Rust 编写的工具，用于检查和管理全局安装的 Cargo 包更新。支持交互式更新、智能预发布版本检测和彩色输出。安装后可以通过 `cargo fresh` 命令使用。工具会自动检测系统语言并相应显示中文或英文界面。

## 功能特性

- 🔍 自动检测已安装的全局 Cargo 包
- 📦 检查每个包的最新版本
- 🎨 彩色输出，清晰显示更新状态
- ⚡ 异步处理，快速检查多个包
- 🛠️ 命令行参数支持，灵活使用
- 🔄 默认交互式更新模式，一键更新包
- 🧠 智能预发布版本检测和询问
- 🌍 自动语言检测（中文/英文）
- 🚀 Cargo 子命令支持（`cargo fresh`）
- 🌐 双语界面，智能语言切换

## 安装

### 从 crates.io 安装（推荐）

```bash
cargo install cargo-fresh
```
or
```bash
# more directly install without complinig
cargo binstall cargo-fresh
```

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

# 生成 shell 补全脚本
cargo fresh completion zsh    # 生成 zsh 补全
cargo fresh completion bash   # 生成 bash 补全
cargo fresh completion fish   # 生成 fish 补全
```

## 输出示例

### 交互模式（默认）

```text
检查全局安装的 Cargo 包更新...
找到 5 个已安装的包

检测到以下包有更新:
稳定版本更新:
  • cargo-outdated (0.16.0 → 0.17.0)
  • devtool (0.2.4 → 0.2.5)

预发布版本更新:
  • mdbook (0.4.52 → 0.5.0-alpha.1) ⚠️ 预发布版本

是否要更新这些包？ [Y/n]: y
是否包含预发布版本更新？ [y/N]: n

选择要更新的包（使用空格选择，回车确认）
> [x] cargo-outdated
> [x] devtool

开始更新选中的包...
正在更新 cargo-outdated...
✅ cargo-outdated 已更新: 0.16.0 → 0.17.0
正在更新 devtool...
✅ devtool 已更新: 0.2.4 → 0.2.5

更新完成！
成功: 2 个包
```

### 非交互模式

```text
检查全局安装的 Cargo 包更新...
找到 5 个已安装的包
mdbook 有更新可用
  当前版本: 0.4.52
  最新版本: 0.5.0-alpha.1

要更新包，请使用: cargo install --force <package_name>
或者移除 --no-interactive 参数进行交互式更新
```

## Shell 补全支持

`cargo-fresh` 支持多种 shell 的自动补全功能，让命令行使用更加便捷。

### 支持的 Shell

- **Zsh** - 完整的补全支持
- **Bash** - 基础补全支持
- **Fish** - 原生补全支持
- **PowerShell** - Windows 补全支持
- **Elvish** - 现代 shell 补全支持

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
# --completion  --help  --include-prerelease  --no-interactive
# --updates-only  --verbose  --version
```

#### Cargo 子命令补全
```bash
cargo <TAB>        # 显示 'fresh' 作为子命令
cargo fresh <TAB>  # 显示所有 fresh 选项和参数
```

## 技术特性

- **异步处理**: 使用 Tokio 异步运行时，快速并发检查多个包
- **智能版本检测**: 自动区分稳定版本和预发布版本
- **交互式界面**: 用户友好的命令行交互体验
- **彩色输出**: 美观的终端输出，清晰的状态显示
- **错误处理**: 完善的错误处理和重试机制
- **类型安全**: Rust 类型系统保证代码安全性
- **进度条**: 实时显示更新进度，提升用户体验
- **Shell 补全**: 支持多种 shell 的自动补全功能
- **语言检测**: 自动系统语言检测和界面适配
- **Cargo 集成**: 原生 cargo 子命令支持，无缝工作流
- **双语支持**: 完整的中英文界面，智能切换
- **模块化架构**: 清晰、可维护的代码结构，分离模块

## 贡献

欢迎贡献代码！请遵循以下步骤：

1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 相关链接

- [Crates.io](https://crates.io/crates/cargo-fresh)
- [GitHub Repository](https://github.com/jenkinpan/pkg-checker-rs)
- [Issues](https://github.com/jenkinpan/pkg-checker-rs/issues)
