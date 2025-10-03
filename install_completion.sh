#!/bin/bash

# pkg-checker Shell 补全安装脚本

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检测 shell 类型
detect_shell() {
    if [ -n "$ZSH_VERSION" ]; then
        echo "zsh"
    elif [ -n "$BASH_VERSION" ]; then
        echo "bash"
    elif [ -n "$FISH_VERSION" ]; then
        echo "fish"
    else
        # 尝试从 SHELL 环境变量检测
        case "$SHELL" in
            */zsh) echo "zsh" ;;
            */bash) echo "bash" ;;
            */fish) echo "fish" ;;
            *) echo "unknown" ;;
        esac
    fi
}

# 获取补全文件路径
get_completion_path() {
    local shell_type="$1"
    local home_dir="$HOME"
    
    case "$shell_type" in
        "zsh")
            echo "$home_dir/.zsh_completions"
            ;;
        "bash")
            echo "$home_dir/.bash_completions"
            ;;
        "fish")
            echo "$home_dir/.config/fish/completions"
            ;;
        *)
            echo ""
            ;;
    esac
}

# 安装补全
install_completion() {
    local shell_type="$1"
    local completion_dir="$2"
    
    print_info "正在为 $shell_type 安装补全..."
    
    # 创建补全目录
    mkdir -p "$completion_dir"
    
    # 生成补全脚本
    local completion_file="$completion_dir/pkg-checker.$shell_type"
    
    if command -v pkg-checker >/dev/null 2>&1; then
        pkg-checker --completion "$shell_type" > "$completion_file"
    else
        print_error "未找到 pkg-checker 命令，请先安装 pkg-checker"
        exit 1
    fi
    
    print_success "补全脚本已安装到: $completion_file"
    
    # 添加到 shell 配置文件
    case "$shell_type" in
        "zsh")
            local zshrc="$HOME/.zshrc"
            if [ -f "$zshrc" ]; then
                if ! grep -q "pkg-checker" "$zshrc"; then
                    echo "" >> "$zshrc"
                    echo "# pkg-checker completion" >> "$zshrc"
                    echo "fpath=(\$HOME/.zsh_completions \$fpath)" >> "$zshrc"
                    echo "autoload -U compinit && compinit" >> "$zshrc"
                    print_success "已添加到 $zshrc"
                else
                    print_warning "$zshrc 中已存在 pkg-checker 配置"
                fi
            else
                print_warning "未找到 $zshrc，请手动添加以下内容："
                echo "fpath=(\$HOME/.zsh_completions \$fpath)"
                echo "autoload -U compinit && compinit"
            fi
            ;;
        "bash")
            local bashrc="$HOME/.bashrc"
            if [ -f "$bashrc" ]; then
                if ! grep -q "pkg-checker" "$bashrc"; then
                    echo "" >> "$bashrc"
                    echo "# pkg-checker completion" >> "$bashrc"
                    echo "source $completion_file" >> "$bashrc"
                    print_success "已添加到 $bashrc"
                else
                    print_warning "$bashrc 中已存在 pkg-checker 配置"
                fi
            else
                print_warning "未找到 $bashrc，请手动添加以下内容："
                echo "source $completion_file"
            fi
            ;;
        "fish")
            print_success "Fish 补全已安装，重启 fish 或运行 'source ~/.config/fish/config.fish'"
            ;;
    esac
}

# 主函数
main() {
    print_info "pkg-checker Shell 补全安装脚本"
    echo
    
    # 检测 shell
    local shell_type=$(detect_shell)
    if [ "$shell_type" = "unknown" ]; then
        print_error "无法检测到支持的 shell 类型"
        print_info "支持的 shell: zsh, bash, fish"
        exit 1
    fi
    
    print_info "检测到 shell: $shell_type"
    
    # 获取补全目录
    local completion_dir=$(get_completion_path "$shell_type")
    if [ -z "$completion_dir" ]; then
        print_error "不支持的 shell: $shell_type"
        exit 1
    fi
    
    # 安装补全
    install_completion "$shell_type" "$completion_dir"
    
    echo
    print_success "安装完成！"
    print_info "请重启终端或运行以下命令来启用补全："
    case "$shell_type" in
        "zsh")
            echo "  source ~/.zshrc"
            ;;
        "bash")
            echo "  source ~/.bashrc"
            ;;
        "fish")
            echo "  source ~/.config/fish/config.fish"
            ;;
    esac
    echo
    print_info "使用方法："
    echo "  pkg-checker <TAB>  # 自动补全命令和选项"
}

# 运行主函数
main "$@"
