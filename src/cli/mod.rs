use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use clap_complete_nushell::Nushell;

#[derive(Parser)]
#[command(name = "pkg-checker")]
#[command(about = "检查全局安装的Cargo包更新")]
#[command(version)]
pub struct Cli {
    /// 显示详细信息
    #[arg(short, long)]
    pub verbose: bool,

    /// 只显示有更新的包
    #[arg(short, long)]
    pub updates_only: bool,

    /// 非交互模式（默认是交互模式）
    #[arg(long)]
    pub no_interactive: bool,

    /// 包含预发布版本（alpha、beta、rc等）
    #[arg(long)]
    pub include_prerelease: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 生成 shell 补全脚本
    Completion {
        /// 要生成补全脚本的 shell
        #[arg(value_enum)]
        shell: ShellType,
    },
}

#[derive(Clone, ValueEnum)]
pub enum ShellType {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell
    Powershell,
    /// Elvish shell
    Elvish,
    /// Nushell
    Nushell,
}

impl Cli {
    pub fn generate_completion(shell: ShellType) {
        let mut cmd = Cli::command();

        match shell {
            ShellType::Bash => {
                generate(Shell::Bash, &mut cmd, "pkg-checker", &mut std::io::stdout());
            }
            ShellType::Zsh => {
                generate(Shell::Zsh, &mut cmd, "pkg-checker", &mut std::io::stdout());
            }
            ShellType::Fish => {
                generate(Shell::Fish, &mut cmd, "pkg-checker", &mut std::io::stdout());
            }
            ShellType::Powershell => {
                generate(Shell::PowerShell, &mut cmd, "pkg-checker", &mut std::io::stdout());
            }
            ShellType::Elvish => {
                generate(Shell::Elvish, &mut cmd, "pkg-checker", &mut std::io::stdout());
            }
            ShellType::Nushell => {
                generate(Nushell, &mut cmd, "pkg-checker", &mut std::io::stdout());
            }
        }
    }
}
