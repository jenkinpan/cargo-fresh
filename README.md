# pkg-checker

[![Crates.io](https://img.shields.io/crates/v/pkg-checker.svg)](https://crates.io/crates/pkg-checker)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

一个用 Rust 编写的工具，用于检查和管理全局安装的 Cargo 包更新。支持交互式更新、智能预发布版本检测和彩色输出。

## 功能特性

- 🔍 自动检测已安装的全局 Cargo 包
- 📦 检查每个包的最新版本
- 🎨 彩色输出，清晰显示更新状态
- ⚡ 异步处理，快速检查多个包
- 🛠️ 命令行参数支持，灵活使用
- 🔄 默认交互式更新模式，一键更新包
- 🧠 智能预发布版本检测和询问

## 安装

### 从 crates.io 安装（推荐）

```bash
cargo install pkg-checker
```

### 从源码安装

```bash
# 克隆项目
git clone https://github.com/your-username/pkg-checker.git
cd pkg-checker

# 构建并安装
cargo install --path .
```

### 从 GitHub 安装

```bash
cargo install --git https://github.com/your-username/pkg-checker.git
```

## 使用方法

### 基本使用

```bash
pkg-checker
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
pkg-checker --verbose

# 只显示有更新的包
pkg-checker --updates-only

# 组合使用
pkg-checker --verbose --updates-only

# 默认交互模式（推荐）
pkg-checker

# 只显示有更新的包（交互模式）
pkg-checker --updates-only

# 非交互模式
pkg-checker --no-interactive

# 包含预发布版本检查（交互模式）
pkg-checker --include-prerelease

# 非交互模式 + 预发布版本
pkg-checker --no-interactive --include-prerelease

# 生成 shell 补全脚本
pkg-checker --completion zsh    # 生成 zsh 补全
pkg-checker --completion bash   # 生成 bash 补全
pkg-checker --completion fish   # 生成 fish 补全
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

`pkg-checker` 支持多种 shell 的自动补全功能，让命令行使用更加便捷。

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
pkg-checker --completion zsh > ~/.zsh_completions/pkg-checker.zsh

# 2. 添加到 zsh 配置
echo 'fpath=($HOME/.zsh_completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc

# 3. 重新加载配置
source ~/.zshrc
```

#### 其他 Shell 安装

```bash
# Bash 补全
pkg-checker --completion bash > ~/.bash_completions/pkg-checker.bash
echo 'source ~/.bash_completions/pkg-checker.bash' >> ~/.bashrc

# Fish 补全
pkg-checker --completion fish > ~/.config/fish/completions/pkg-checker.fish

# PowerShell 补全
pkg-checker --completion powershell > pkg-checker.ps1
```

### 使用方法

安装完成后，在命令行中输入 `pkg-checker` 然后按 `TAB` 键即可看到自动补全选项：

```bash
pkg-checker <TAB>
# 显示所有可用选项：
# --completion  --help  --include-prerelease  --no-interactive
# --updates-only  --verbose  --version
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

- [Crates.io](https://crates.io/crates/pkg-checker)
- [GitHub Repository](https://github.com/your-username/pkg-checker)
- [Issues](https://github.com/your-username/pkg-checker/issues)
