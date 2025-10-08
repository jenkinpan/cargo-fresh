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
# 会显示: completion  help

# 输入 completion 后按 Tab 键
pkg-checker completion <TAB>
# 会显示: bash  zsh  fish  powershell  elvish  nushell
```

## 测试补全功能

### 测试步骤

1. **安装补全脚本** - 按照上面的安装说明安装对应 shell 的补全脚本
2. **重新启动终端** - 或者重新加载 shell 配置
3. **测试主命令补全**:
   ```bash
   pkg-checker <TAB>
   # 应该显示: completion  help
   ```
4. **测试子命令补全**:
   ```bash
   pkg-checker completion <TAB>
   # 应该显示: bash  zsh  fish  powershell  elvish  nushell
   ```

### 故障排除

如果补全不工作，请检查：

1. **补全脚本是否正确安装** - 确认文件在正确的位置
2. **shell 配置是否正确** - 确认补全脚本被正确加载
3. **是否需要重新启动终端** - 某些 shell 需要重启才能生效
4. **检查 shell 版本** - 确保 shell 版本支持补全功能

## 已知问题

### Fish Shell 补全问题

由于 clap_complete 对 fish shell 的支持限制，自动生成的 fish 补全脚本可能缺少 shell 选项补全。

**解决方案：**

1. **使用修复后的补全脚本**：
   ```bash
   # 生成修复后的 fish 补全脚本
   cargo run -- completion fish > /tmp/pkg-checker.fish
   
   # 手动添加 shell 选项补全（需要手动编辑）
   # 或者使用我们提供的修复版本
   ```

2. **手动安装修复版本**：
   ```bash
   # 下载修复后的补全脚本
   # 安装到 fish 补全目录
   cp pkg-checker-fixed.fish ~/.config/fish/completions/pkg-checker.fish
   ```

3. **其他 shell 不受影响**：
   - bash, zsh, powershell, elvish, nushell 的补全功能完全正常
   - 只有 fish shell 需要特殊处理

## 注意事项

- 确保你的 shell 支持补全功能
- 某些 shell 可能需要重新启动或重新加载配置
- Fish shell 用户需要手动安装修复后的补全脚本
- 补全脚本会随着 `pkg-checker` 的更新而更新，建议定期重新生成
