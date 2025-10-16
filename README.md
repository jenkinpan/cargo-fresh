# cargo-fresh

[![Crates.io](https://img.shields.io/crates/v/cargo-fresh.svg)](https://crates.io/crates/cargo-fresh)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

<div align="center">

**Language / 语言**

[![English](https://img.shields.io/badge/English-Current-blue?style=for-the-badge)](README.md) [![中文](https://img.shields.io/badge/中文-中文版-green?style=for-the-badge)](README.zh.md)

</div>

---

A Rust tool for checking and updating globally installed Cargo packages with interactive mode and smart prerelease detection. After installation, you can use it via the `cargo fresh` command. The tool automatically detects your system language and displays the interface in Chinese or English accordingly.

## Features

- 🔍 Automatically detect globally installed Cargo packages
- 📦 Check for the latest version of each package
- 🎨 Colored output with clear update status display
- ⚡ Asynchronous processing for fast checking of multiple packages
- 🛠️ Command-line argument support for flexible usage
- 🔄 Default interactive update mode with one-click package updates
- 🧠 Smart prerelease version detection and prompting
- 🌍 Automatic language detection (Chinese/English)
- 🚀 Cargo subcommand support (`cargo fresh`)
- 🌐 Bilingual interface with smart language switching

## Installation

### Install from crates.io (Recommended)

```bash
cargo install cargo-fresh
```
or
```bash
# directly install without complinig
cargo binstall cargo-fresh
```

### Install from source

```bash
# Clone the repository
git clone https://github.com/jenkinpan/cargo-fresh.git
cd cargo-fresh

# Build and install
cargo install --path .
```

### Install from GitHub

```bash
cargo install --git https://github.com/jenkinpan/cargo-fresh.git
```

## Language Support

The tool automatically detects your system language and displays the interface accordingly:

- **Chinese Environment**: Automatically displays Chinese interface
- **English Environment**: Automatically displays English interface
- **Language Detection**: Based on system environment variables (LANG, LC_ALL, LC_CTYPE)

You can also manually override the language by setting environment variables:

```bash
# Force English interface
LANG=en_US.UTF-8 cargo fresh

# Force Chinese interface  
LANG=zh_CN.UTF-8 cargo fresh
```

## Usage

### Basic Usage

After installation, you can use it in two ways:

```bash
# Method 1: As a cargo subcommand (recommended)
cargo fresh

# Method 2: Direct invocation
cargo-fresh
```

### Command Line Options

- `-v, --verbose`: Show detailed information
- `-u, --updates-only`: Show only packages with updates
- `--no-interactive`: Non-interactive mode (default is interactive mode)
- `--include-prerelease`: Include prerelease versions (alpha, beta, rc, etc.)
- `-h, --help`: Show help information
- `-V, --version`: Show version information

### Examples

```bash
# Check all packages and show detailed information
cargo fresh --verbose

# Show only packages with updates
cargo fresh --updates-only

# Combine options
cargo fresh --verbose --updates-only

# Default interactive mode (recommended)
cargo fresh

# Show only packages with updates (interactive mode)
cargo fresh --updates-only

# Non-interactive mode
cargo fresh --no-interactive

# Include prerelease version checks (interactive mode)
cargo fresh --include-prerelease

# Non-interactive mode + prerelease versions
cargo fresh --no-interactive --include-prerelease

# Generate shell completion scripts
cargo fresh completion zsh    # Generate zsh completion
cargo fresh completion bash   # Generate bash completion
cargo fresh completion fish   # Generate fish completion

# Generate cargo fresh subcommand completion
cargo fresh completion zsh --cargo-fresh    # Generate cargo fresh zsh completion
cargo fresh completion bash --cargo-fresh   # Generate cargo fresh bash completion
```

## Output Examples

### Interactive Mode (Default)

```text
Checking for updates to globally installed Cargo packages...
Found 5 installed packages

The following packages have updates available:
Stable version updates:
  • cargo-outdated (0.16.0 → 0.17.0)
  • devtool (0.2.4 → 0.2.5)

Prerelease version updates:
  • mdbook (0.4.52 → 0.5.0-alpha.1) ⚠️ Prerelease version

Do you want to update these packages? [Y/n]: y
Include prerelease version updates? [y/N]: n

Select packages to update (use space to select, enter to confirm)
> [x] cargo-outdated
> [x] devtool

Starting to update selected packages...
Updating cargo-outdated...
✅ cargo-outdated updated: 0.16.0 → 0.17.0
Updating devtool...
✅ devtool updated: 0.2.4 → 0.2.5

Update completed!
Success: 2 packages
```

### Non-Interactive Mode

```text
Checking for updates to globally installed Cargo packages...
Found 5 installed packages
mdbook has updates available
  Current version: 0.4.52
  Latest version: 0.5.0-alpha.1

To update packages, use: cargo install --force <package_name>
Or remove --no-interactive flag for interactive updates
```

## Shell Completion Support

`cargo-fresh` supports automatic completion for multiple shells, making command-line usage more convenient.

### Supported Shells

- **Zsh** - Full completion support
- **Bash** - Basic completion support
- **Fish** - Native completion support
- **PowerShell** - Windows completion support
- **Elvish** - Modern shell completion support

### Installing Completions

#### Manual Installation

```bash
# 1. Generate completion script
cargo fresh completion zsh > ~/.zsh_completions/cargo-fresh.zsh

# 2. Add to zsh configuration
echo 'fpath=($HOME/.zsh_completions $fpath)' >> ~/.zshrc
echo 'autoload -U compinit && compinit' >> ~/.zshrc

# 3. Reload configuration
source ~/.zshrc
```

#### Cargo Fresh Subcommand Completion

For `cargo fresh` subcommand completion:

```bash
# Generate cargo fresh subcommand completion
cargo fresh completion zsh --cargo-fresh > cargo-fresh-completion.zsh
cargo fresh completion bash --cargo-fresh > cargo-fresh-completion.bash

# Install cargo fresh completion
source cargo-fresh-completion.zsh  # For zsh
source cargo-fresh-completion.bash # For bash
```

#### Other Shell Installation

```bash
# Bash completion
cargo fresh completion bash > ~/.bash_completions/cargo-fresh.bash
echo 'source ~/.bash_completions/cargo-fresh.bash' >> ~/.bashrc

# Fish completion
cargo fresh completion fish > ~/.config/fish/completions/cargo-fresh.fish

# PowerShell completion
cargo fresh completion powershell > cargo-fresh.ps1
```

### Usage

After installation, you can use auto-completion in two ways:

#### Direct Command Completion
```bash
cargo fresh <TAB>
# Shows all available options:
# --completion  --help  --include-prerelease  --no-interactive
# --updates-only  --verbose  --version
```

#### Cargo Subcommand Completion
```bash
cargo <TAB>        # Shows 'fresh' as a subcommand
cargo fresh <TAB>  # Shows all fresh options and parameters
```

## Technical Features

- **Asynchronous Processing**: Uses Tokio async runtime for fast concurrent checking of multiple packages
- **Smart Version Detection**: Automatically distinguishes between stable and prerelease versions
- **Interactive Interface**: User-friendly command-line interaction experience
- **Colored Output**: Beautiful terminal output with clear status display
- **Error Handling**: Comprehensive error handling and retry mechanisms
- **Type Safety**: Rust type system ensures code safety
- **Progress Bars**: Real-time update progress display for better user experience
- **Shell Completion**: Auto-completion support for multiple shells
- **Language Detection**: Automatic system language detection and interface adaptation
- **Cargo Integration**: Native cargo subcommand support for seamless workflow
- **Bilingual Support**: Complete Chinese and English interface with smart switching
- **Modular Architecture**: Clean, maintainable code structure with separate modules

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the project
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Create a Pull Request

## License

This project is licensed under the Apache 2.0 License. See the [LICENSE](LICENSE) file for complete license terms.

### License Summary

The Apache 2.0 License is a permissive open source license that allows you to:

- ✅ **Commercial use** - Use in commercial projects
- ✅ **Modification** - Modify the source code
- ✅ **Distribution** - Distribute original or modified code
- ✅ **Private use** - Use privately
- ✅ **Patent use** - Use related patents
- ✅ **Patent grant** - Automatic patent license grant

**Main requirements**:
- Include the original license and copyright notice when distributing
- Must state changes made to the source code
- Cannot use project name, trademarks, or product names for promotion

### Copyright Information

Copyright (c) 2025 Jenkin Pan

This project is open source under the Apache 2.0 License. See the [LICENSE](LICENSE) file for details.

## Related Links

- [Crates.io](https://crates.io/crates/cargo-fresh)
- [GitHub Repository](https://github.com/jenkinpan/pkg-checker-rs)
- [Issues](https://github.com/jenkinpan/pkg-checker-rs/issues)
