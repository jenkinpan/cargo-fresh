use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use clap_complete_nushell::Nushell;

#[derive(Parser)]
#[command(name = "cargo-fresh")]
#[command(about = "Check and update globally installed Cargo packages")]
#[command(version)]
pub struct Cli {
    /// Show detailed information
    #[arg(short, long)]
    pub verbose: bool,

    /// Show only packages with updates
    #[arg(short, long)]
    pub updates_only: bool,

    /// Non-interactive mode (default is interactive mode)
    #[arg(long)]
    pub no_interactive: bool,

    /// Include prerelease versions (alpha, beta, rc, etc.)
    #[arg(long)]
    pub include_prerelease: bool,

    /// Batch mode - automatically update all packages without confirmation
    #[arg(long)]
    pub batch: bool,

    /// Filter packages by name pattern (supports glob patterns)
    #[arg(long)]
    pub filter: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Generate shell completion scripts
    Completion {
        /// Shell to generate completion script for
        #[arg(value_enum)]
        shell: ShellType,
        /// Generate completion for cargo fresh subcommand
        #[arg(long)]
        cargo_fresh: bool,
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
    /// 创建标准的 cargo-fresh 命令结构
    fn create_cargo_fresh_command() -> clap::Command {
        clap::Command::new("cargo-fresh")
            .about("Check and update globally installed Cargo packages")
            .arg(
                clap::Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("Show detailed information"),
            )
            .arg(
                clap::Arg::new("updates-only")
                    .short('u')
                    .long("updates-only")
                    .help("Show only packages with updates"),
            )
            .arg(
                clap::Arg::new("no-interactive")
                    .long("no-interactive")
                    .help("Non-interactive mode (default is interactive mode)"),
            )
            .arg(
                clap::Arg::new("include-prerelease")
                    .long("include-prerelease")
                    .help("Include prerelease versions (alpha, beta, rc, etc.)"),
            )
            .arg(
                clap::Arg::new("batch")
                    .long("batch")
                    .help("Batch mode - automatically update all packages without confirmation"),
            )
            .arg(
                clap::Arg::new("filter")
                    .long("filter")
                    .help("Filter packages by name pattern (supports glob patterns)")
                    .value_name("PATTERN"),
            )
            .subcommand(
                clap::Command::new("completion")
                    .about("Generate shell completion scripts")
                    .arg(
                        clap::Arg::new("shell")
                            .help("Shell to generate completion script for")
                            .value_parser(clap::value_parser!(ShellType)),
                    )
                    .arg(
                        clap::Arg::new("cargo-fresh")
                            .long("cargo-fresh")
                            .help("Generate completion for cargo fresh subcommand")
                            .action(clap::ArgAction::SetTrue),
                    ),
            )
    }

    /// 生成补全脚本的通用方法
    fn generate_completion_for_shell(shell: ShellType, cmd: &mut clap::Command, name: &str) {
        let shell_type = match shell {
            ShellType::Bash => Shell::Bash,
            ShellType::Zsh => Shell::Zsh,
            ShellType::Fish => Shell::Fish,
            ShellType::Powershell => Shell::PowerShell,
            ShellType::Elvish => Shell::Elvish,
            ShellType::Nushell => return generate(Nushell, cmd, name, &mut std::io::stdout()),
        };
        generate(shell_type, cmd, name, &mut std::io::stdout());
    }

    pub fn generate_completion(shell: ShellType) {
        let mut cmd = Self::create_cargo_fresh_command();
        Self::generate_completion_for_shell(shell, &mut cmd, "cargo-fresh");
    }

    /// 生成 cargo fresh 子命令的补全脚本
    pub fn generate_cargo_fresh_completion(shell: ShellType) {
        let mut cargo_cmd = clap::Command::new("cargo")
            .about("Rust's package manager")
            .subcommand(
                Self::create_cargo_fresh_command()
                    .name("fresh")
                    .about("Check and update globally installed Cargo packages"),
            );

        Self::generate_completion_for_shell(shell, &mut cargo_cmd, "cargo");
    }
}
