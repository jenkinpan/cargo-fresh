#!/bin/bash

# pkg-checker Shell 补全智能安装脚本

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

# 检查 pkg-checker 是否可用
check_pkg_checker() {
    # 尝试不同的 pkg-checker 路径
    if command -v pkg-checker >/dev/null 2>&1; then
        print_success "找到 pkg-checker 命令 (系统路径)"
        return 0
    elif [ -f "./target/release/pkg-checker" ]; then
        print_success "找到 pkg-checker 命令 (release 版本)"
        return 0
    elif [ -f "./target/debug/pkg-checker" ]; then
        print_success "找到 pkg-checker 命令 (debug 版本)"
        return 0
    else
        print_error "未找到 pkg-checker 命令"
        print_info "请先安装 pkg-checker："
        echo "  cargo install pkg-checker"
        echo "  或者从源码构建：cargo build --release"
        exit 1
    fi
}

# 安装单个 shell 的补全
install_completion() {
    local shell_type="$1"
    local completion_dir="$2"
    
    print_info "正在为 $shell_type 安装补全..."
    
    # 创建补全目录
    mkdir -p "$completion_dir"
    
    # 生成补全脚本
    local completion_file="$completion_dir/pkg-checker.$shell_type"
    
    # 尝试不同的 pkg-checker 路径，优先使用本地构建版本
    local pkg_checker_cmd=""
    if [ -f "./target/release/pkg-checker" ]; then
        pkg_checker_cmd="./target/release/pkg-checker"
    elif [ -f "./target/debug/pkg-checker" ]; then
        pkg_checker_cmd="./target/debug/pkg-checker"
    elif command -v pkg-checker >/dev/null 2>&1; then
        # 检查系统版本是否支持补全
        if pkg-checker --help 2>&1 | grep -q "completion"; then
            pkg_checker_cmd="pkg-checker"
        else
            print_error "系统安装的 pkg-checker 版本不支持补全功能"
            print_info "请使用本地构建版本或更新到最新版本"
            return 1
        fi
    else
        print_error "未找到 pkg-checker 可执行文件"
        return 1
    fi
    
    print_info "使用命令: $pkg_checker_cmd --completion $shell_type"
    if $pkg_checker_cmd --completion "$shell_type" > "$completion_file" 2>/dev/null && [ -s "$completion_file" ]; then
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
        return 0
    else
        print_error "生成 $shell_type 补全失败"
        return 1
    fi
}

# 自动检测并安装所有支持的 shell 补全
auto_install_all() {
    print_info "自动检测并安装所有支持的 shell 补全..."
    
    local shells=("zsh" "bash" "fish")
    local installed_count=0
    
    for shell in "${shells[@]}"; do
        local completion_dir=$(get_completion_path "$shell")
        if [ -n "$completion_dir" ]; then
            if install_completion "$shell" "$completion_dir"; then
                ((installed_count++))
            fi
        fi
    done
    
    print_success "共安装了 $installed_count 个 shell 的补全"
}

# 显示使用帮助
show_help() {
    echo "pkg-checker Shell 补全安装脚本"
    echo
    echo "用法:"
    echo "  $0                    # 自动检测并安装所有支持的 shell 补全"
    echo "  $0 [shell]            # 为指定 shell 安装补全"
    echo "  $0 --help             # 显示此帮助信息"
    echo
    echo "支持的 shell:"
    echo "  zsh, bash, fish"
    echo
    echo "示例:"
    echo "  $0                    # 自动安装所有"
    echo "  $0 zsh               # 只安装 zsh 补全"
    echo "  $0 bash              # 只安装 bash 补全"
}

# 主函数
main() {
    # 检查参数
    if [ "$1" = "--help" ] || [ "$1" = "-h" ]; then
        show_help
        exit 0
    fi
    
    print_info "pkg-checker Shell 补全智能安装脚本"
    echo
    
    # 检查 pkg-checker 是否可用
    check_pkg_checker
    
    # 检查是否指定了特定 shell
    if [ $# -gt 0 ]; then
        local shell_type="$1"
        print_info "指定安装 shell: $shell_type"
        
        # 获取补全目录
        local completion_dir=$(get_completion_path "$shell_type")
        if [ -z "$completion_dir" ]; then
            print_error "不支持的 shell: $shell_type"
            print_info "支持的 shell: zsh, bash, fish"
            exit 1
        fi
        
        # 安装补全
        if install_completion "$shell_type" "$completion_dir"; then
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
        else
            print_error "安装失败"
            exit 1
        fi
    else
        # 自动检测当前 shell
        local current_shell=$(detect_shell)
        if [ "$current_shell" != "unknown" ]; then
            print_info "检测到当前 shell: $current_shell"
            print_info "是否要为所有支持的 shell 安装补全？(推荐) [Y/n]"
            read -r response
            if [[ "$response" =~ ^[Nn]$ ]]; then
                # 只安装当前 shell
                local completion_dir=$(get_completion_path "$current_shell")
                if [ -n "$completion_dir" ]; then
                    if install_completion "$current_shell" "$completion_dir"; then
                        echo
                        print_success "安装完成！"
                        print_info "请重启终端或运行以下命令来启用补全："
                        case "$current_shell" in
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
                    else
                        print_error "安装失败"
                        exit 1
                    fi
                else
                    print_error "不支持的 shell: $current_shell"
                    exit 1
                fi
            else
                # 安装所有支持的 shell
                auto_install_all
            fi
        else
            print_warning "无法检测到当前 shell，将安装所有支持的 shell 补全"
            auto_install_all
        fi
    fi
    
    echo
    print_info "使用方法："
    echo "  pkg-checker <TAB>  # 自动补全命令和选项"
    echo
    print_info "如需重新安装，请运行："
    echo "  $0 [shell_name]  # 指定 shell，如: $0 zsh"
}

# 运行主函数
main "$@"