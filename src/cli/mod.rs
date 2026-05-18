use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
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

    /// Filter packages by name pattern (supports glob patterns: *, ?, [abc])
    #[arg(long)]
    pub filter: Option<String>,

    /// Exclude packages by glob pattern (repeatable)
    #[arg(long, action = clap::ArgAction::Append)]
    pub exclude: Vec<String>,

    /// Print commands that would run but don't execute them
    #[arg(long)]
    pub dry_run: bool,

    /// Override sparse index base URL (e.g. https://mirrors.ustc.edu.cn/crates.io-index/).
    /// Default: read from $CARGO_HOME/config.toml [source.crates-io] replace-with,
    /// fall back to https://index.crates.io.
    #[arg(long, value_name = "URL")]
    pub registry_url: Option<String>,

    /// Output format. `human` (default) prints cargo-style status lines;
    /// `json` emits a single machine-readable object and disables colors, spinners,
    /// and interactive prompts. Useful in CI.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, value_name = "FORMAT")]
    pub format: OutputFormat,

    /// Disable the `cargo search` fallback when the sparse index request fails.
    /// `cargo search` is slow (spawns a cargo subprocess, parses output) and brittle.
    /// You can also set `CARGO_FRESH_NO_FALLBACK=1` to achieve the same effect.
    #[arg(long)]
    pub no_cargo_search_fallback: bool,

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

/// 输出格式。Human 是 cargo 风格的状态行；Json 用于 CI / 脚本消费。
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Cargo-style status lines (default)
    Human,
    /// Machine-readable JSON document (one line, no colors, no spinners, no prompts)
    Json,
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

    /// 生成顶层 `cargo-fresh` 的补全脚本，直接复用 derive 出来的 Command。
    pub fn generate_completion(shell: ShellType) {
        let mut cmd = Self::command();
        Self::generate_completion_for_shell(shell, &mut cmd, "cargo-fresh");
    }

    /// 生成 `cargo fresh` 子命令的补全脚本——把同一个 derive 出来的 Command
    /// 重命名为 "fresh" 后挂到 `cargo` 下，避免和顶层补全双重维护。
    pub fn generate_cargo_fresh_completion(shell: ShellType) {
        let fresh = Self::command()
            .name("fresh")
            .about("Check and update globally installed Cargo packages");
        let mut cargo_cmd = clap::Command::new("cargo")
            .about("Rust's package manager")
            .subcommand(fresh);

        Self::generate_completion_for_shell(shell, &mut cargo_cmd, "cargo");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_command_builds() {
        // CommandFactory 派生出来的 Command 至少能成功构造并自检
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_parses_basic_flags() {
        let cli = Cli::try_parse_from([
            "cargo-fresh",
            "--verbose",
            "--batch",
            "--filter",
            "cargo-*",
            "--exclude",
            "foo",
            "--exclude",
            "bar",
        ])
        .expect("parse");
        assert!(cli.verbose);
        assert!(cli.batch);
        assert_eq!(cli.filter.as_deref(), Some("cargo-*"));
        assert_eq!(cli.exclude, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn cli_completion_subcommand() {
        let cli =
            Cli::try_parse_from(["cargo-fresh", "completion", "bash", "--cargo-fresh"]).expect("parse");
        match cli.command {
            Some(Commands::Completion { cargo_fresh, .. }) => assert!(cargo_fresh),
            _ => panic!("expected completion subcommand"),
        }
    }
}
