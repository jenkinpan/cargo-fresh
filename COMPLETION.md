# Shell 补全支持

`pkg-checker` 支持多种 shell 的自动补全功能，可以通过 `--completion` 参数生成相应的补全脚本。

## 支持的 Shell

- **bash** - Bash shell 补全
- **zsh** - Zsh shell 补全  
- **fish** - Fish shell 补全
- **powershell** - PowerShell 补全
- **elvish** - Elvish shell 补全
- **nushell** - Nushell 补全

## 使用方法

### 生成补全脚本

```bash
# 生成 bash 补全脚本
pkg-checker completion bash > pkg-checker.bash

# 生成 zsh 补全脚本
pkg-checker completion zsh > _pkg-checker

# 生成 fish 补全脚本
pkg-checker completion fish > pkg-checker.fish

# 生成 PowerShell 补全脚本
pkg-checker completion powershell > pkg-checker.ps1

# 生成 elvish 补全脚本
pkg-checker completion elvish > pkg-checker.elv

# 生成 nushell 补全脚本
pkg-checker completion nushell > pkg-checker.nu
```

### 安装补全脚本

#### Bash
```bash
# 方法1: 添加到 ~/.bashrc
echo 'source /path/to/pkg-checker.bash' >> ~/.bashrc

# 方法2: 复制到系统目录
sudo cp pkg-checker.bash /etc/bash_completion.d/
```

#### Zsh
```bash
# 复制到 fpath 目录
cp _pkg-checker ~/.zsh/completions/

# 确保在 ~/.zshrc 中启用补全
autoload -U compinit && compinit
```

#### Fish
```bash
# 复制到 fish 补全目录
mkdir -p ~/.config/fish/completions/
cp pkg-checker.fish ~/.config/fish/completions/
```

#### PowerShell
```powershell
# 在 PowerShell 中执行
. ./pkg-checker.ps1
```

#### Elvish
```bash
# 复制到 elvish 配置目录
mkdir -p ~/.config/elvish/lib
cp pkg-checker.elv ~/.config/elvish/lib/
```

#### Nushell
```bash
# 复制到 nushell 配置目录
mkdir -p ~/.config/nushell/completions
cp pkg-checker.nu ~/.config/nushell/completions/

# 或者在 config.nu 中手动加载
# source ~/.config/nushell/completions/pkg-checker.nu
```

## 补全功能

生成的补全脚本支持以下功能：

- **参数补全**: 自动补全所有命令行参数
- **选项补全**: 支持短选项 (`-v`) 和长选项 (`--verbose`)
- **智能提示**: 显示参数说明和帮助信息

## 示例

安装补全脚本后，你可以享受以下体验：

```bash
# 输入 pkg-checker 后按 Tab 键
pkg-checker <TAB>
# 会显示所有可用选项

# 输入部分参数后按 Tab 键
pkg-checker completion <TAB>
# 会显示支持的 shell 列表
```

## 注意事项

- 确保你的 shell 支持补全功能
- 某些 shell 可能需要重新启动或重新加载配置
- 补全脚本会随着 `pkg-checker` 的更新而更新，建议定期重新生成
