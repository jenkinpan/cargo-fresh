#!/bin/bash

echo "构建 Cargo Update..."

# 检查是否安装了 Rust
if ! command -v cargo &> /dev/null; then
    echo "错误: 未找到 cargo 命令。请先安装 Rust。"
    echo "访问 https://rustup.rs/ 安装 Rust"
    exit 1
fi

# 构建项目
echo "正在构建项目..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ 构建成功！"
    echo ""
    echo "使用方法："
    echo "  ./target/release/cargo-update                    # 默认交互模式（推荐）"
    echo "  ./target/release/cargo-update --help"
    echo "  ./target/release/cargo-update --verbose"
    echo "  ./target/release/cargo-update --updates-only"
    echo "  ./target/release/cargo-update --no-interactive  # 非交互模式"
    echo "  ./target/release/cargo-update --include-prerelease  # 包含预发布版本"
    echo ""
    echo "安装到全局："
    echo "  cargo install --path ."
else
    echo "❌ 构建失败！"
    exit 1
fi

