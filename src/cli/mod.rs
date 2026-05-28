use std::io::{IsTerminal, Write};

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

    /// Probe each update candidate with `cargo binstall --dry-run` during the
    /// check phase, and mark whether binstall would fetch a prebuilt binary
    /// (fast) or fall back to compiling from source (slow). Off by default —
    /// each probe spawns cargo and hits the network (~10s/package, run
    /// concurrently). Requires cargo-binstall to be installed; otherwise the
    /// run prints a hint and proceeds without markers.
    #[arg(long)]
    pub check_binstall: bool,

    /// Maximum number of packages to update concurrently. `0` means unlimited
    /// (one task per selected package). Default 4 — downloads are network-bound
    /// and the inner HEAD-probe pool already caps connection fan-out, so 4
    /// balances throughput against GitHub-CDN friendliness. `cargo install`
    /// fallbacks naturally serialize on cargo's $CARGO_HOME lock regardless of
    /// this value.
    #[arg(short = 'j', long, default_value_t = 4, value_name = "N")]
    pub jobs: u32,

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
    /// Show the man page
    ///
    /// When stdout is a TTY, renders via the system `man` command (with pager).
    /// When redirected/piped, emits raw roff to stdout — save it to MANPATH
    /// (`cargo fresh man > ~/.local/share/man/man1/cargo-fresh.1`) or pipe to
    /// `mandoc` / `groff -Tutf8 -man`.
    Man,
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

    /// 把同一个 derive 出来的 Command 渲染成 roff。
    ///
    /// stdout 是 TTY 时：写到临时文件并 `exec man <tmpfile>`，让系统 man 处理渲染+分页。
    /// stdout 不是 TTY 时（重定向/管道/CI）：把 raw roff 写到 stdout，保持
    /// `cargo fresh man > cargo-fresh.1` 与 `cargo fresh man | mandoc` 这条路径不破。
    pub fn generate_man() -> std::io::Result<()> {
        let cmd = Self::command();
        let man = clap_mangen::Man::new(cmd);

        if !std::io::stdout().is_terminal() {
            return man.render(&mut std::io::stdout());
        }

        let mut buf = Vec::new();
        man.render(&mut buf)?;

        let tmp_dir = std::env::temp_dir();
        let pid = std::process::id();
        let tmp_path = tmp_dir.join(format!("cargo-fresh-{pid}.1"));
        {
            let mut f = std::fs::File::create(&tmp_path)?;
            f.write_all(&buf)?;
        }

        let status = std::process::Command::new("man").arg(&tmp_path).status();
        let _ = std::fs::remove_file(&tmp_path);
        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(_) | Err(_) => {
                // man 不可用或失败时退化为 raw roff，便于用户改用管道
                std::io::stdout().write_all(&buf)
            }
        }
    }

    /// 当用户在 TTY 里跑 `completion fish --cargo-fresh` 时，往 stderr 提示正确的
    /// 安装路径——直接 `> ~/.config/fish/completions/cargo-fresh.fish` 是常见的失误
    /// （那个文件名只在输入 `cargo-fresh<TAB>` 时被 fish 自动加载，不会响应
    /// `cargo fresh<TAB>`）。stdout 留给重定向，提示走 stderr 不污染管道。
    pub fn maybe_hint_fish_install(shell: &ShellType, cargo_fresh: bool) {
        if !matches!(shell, ShellType::Fish) || !cargo_fresh {
            return;
        }
        if !std::io::stderr().is_terminal() {
            return;
        }
        let _ = writeln!(
            std::io::stderr(),
            "hint: install to ~/.config/fish/completions/cargo.fish (merges with cargo's own completion) \
             or ~/.config/fish/conf.d/cargo-fresh.fish (eager-loaded at shell start). \
             Saving to ~/.config/fish/completions/cargo-fresh.fish will NOT enable `cargo fresh<TAB>`."
        );
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

    #[test]
    fn parses_jobs_long() {
        let cli = Cli::try_parse_from(["cargo-fresh", "--jobs", "8"]).unwrap();
        assert_eq!(cli.jobs, 8);
    }

    #[test]
    fn parses_jobs_short() {
        let cli = Cli::try_parse_from(["cargo-fresh", "-j", "2"]).unwrap();
        assert_eq!(cli.jobs, 2);
    }

    #[test]
    fn jobs_default_is_four() {
        let cli = Cli::try_parse_from(["cargo-fresh"]).unwrap();
        assert_eq!(cli.jobs, 4);
    }

    #[test]
    fn jobs_zero_is_accepted() {
        let cli = Cli::try_parse_from(["cargo-fresh", "-j", "0"]).unwrap();
        assert_eq!(cli.jobs, 0);
    }
}
