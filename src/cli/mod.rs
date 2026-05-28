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

    /// Probe each update candidate with cargo-fresh's own downloader to mark
    /// whether prebuilt binaries are available (fast) or it'd fall back to
    /// compiling from source (slow). Replaces the older `--check-binstall`
    /// flag — same intent, but uses the same HEAD-probe logic as the actual
    /// update path so the verdict matches what update would do. Off by
    /// default — each candidate does a few HEAD requests.
    #[arg(long)]
    pub check_prebuilt: bool,

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
        /// Write the completion to the standard config location instead of stdout
        /// (currently fish-only; prompts before overwriting existing files)
        #[arg(long)]
        install: bool,
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

/// `--install` 的结果——调用者据此决定打 `Installed` 还是 `Skip` 状态行。
pub enum InstallOutcome {
    Written(std::path::PathBuf),
    Skipped(std::path::PathBuf),
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
    /// 生成补全脚本的通用方法。`out` 让调用者决定写到 stdout 还是缓冲区。
    fn render_completion_into(shell: &ShellType, cmd: &mut clap::Command, name: &str, out: &mut dyn Write) {
        let shell_type = match shell {
            ShellType::Bash => Shell::Bash,
            ShellType::Zsh => Shell::Zsh,
            ShellType::Fish => Shell::Fish,
            ShellType::Powershell => Shell::PowerShell,
            ShellType::Elvish => Shell::Elvish,
            ShellType::Nushell => return generate(Nushell, cmd, name, out),
        };
        generate(shell_type, cmd, name, out);
    }

    fn build_cargo_fresh_command() -> clap::Command {
        let fresh = Self::command()
            .name("fresh")
            .about("Check and update globally installed Cargo packages");
        clap::Command::new("cargo")
            .about("Rust's package manager")
            .subcommand(fresh)
    }

    /// 生成顶层 `cargo-fresh` 的补全脚本，直接复用 derive 出来的 Command。
    pub fn generate_completion(shell: ShellType) {
        let mut cmd = Self::command();
        Self::render_completion_into(&shell, &mut cmd, "cargo-fresh", &mut std::io::stdout());
    }

    /// 生成 `cargo fresh` 子命令的补全脚本——把同一个 derive 出来的 Command
    /// 重命名为 "fresh" 后挂到 `cargo` 下，避免和顶层补全双重维护。
    pub fn generate_cargo_fresh_completion(shell: ShellType) {
        let mut cargo_cmd = Self::build_cargo_fresh_command();
        Self::render_completion_into(&shell, &mut cargo_cmd, "cargo", &mut std::io::stdout());
    }

    /// 把补全脚本渲染到内存，供 `--install` 路径写文件。
    fn render_completion_to_bytes(shell: &ShellType, cargo_fresh: bool) -> Vec<u8> {
        let mut buf = Vec::new();
        if cargo_fresh {
            let mut cargo_cmd = Self::build_cargo_fresh_command();
            Self::render_completion_into(shell, &mut cargo_cmd, "cargo", &mut buf);
        } else {
            let mut cmd = Self::command();
            Self::render_completion_into(shell, &mut cmd, "cargo-fresh", &mut buf);
        }
        buf
    }

    /// 计算 `--install` 写入路径。目前只支持 fish——其它 shell 的标准路径差异太大
    /// （zsh 走 $fpath、bash 走 XDG_DATA 或 /etc/bash_completion.d），自动选址容易写错地方。
    pub fn install_target_path(shell: &ShellType, cargo_fresh: bool) -> Option<std::path::PathBuf> {
        match shell {
            ShellType::Fish => {
                let base = std::env::var_os("XDG_CONFIG_HOME")
                    .map(std::path::PathBuf::from)
                    .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
                let file = if cargo_fresh { "cargo.fish" } else { "cargo-fresh.fish" };
                Some(base.join("fish").join("completions").join(file))
            }
            _ => None,
        }
    }

    /// 把补全脚本写到 [`install_target_path`] 给出的位置。已存在文件时通过 dialoguer
    /// 询问是否覆盖；非 TTY / 用户拒绝时返回 `Ok(InstallOutcome::Skipped)` 让调用者打 skip 状态。
    /// 不支持的 shell 直接返回错误，调用者照常打 Failed。
    /// `language` 用于把覆盖提示和错误信息按用户语言输出。
    pub fn install_completion(
        shell: &ShellType,
        cargo_fresh: bool,
        language: crate::locale::Language,
    ) -> anyhow::Result<InstallOutcome> {
        let target = Self::install_target_path(shell, cargo_fresh).ok_or_else(|| {
            anyhow::anyhow!("{}", language.get_text("completion_install_unsupported"))
        })?;

        let bytes = Self::render_completion_to_bytes(shell, cargo_fresh);

        if target.exists() {
            if !std::io::stderr().is_terminal() {
                return Ok(InstallOutcome::Skipped(target));
            }
            let prompt = language
                .get_text("completion_overwrite_prompt")
                .replace("{}", &target.display().to_string());
            let proceed = dialoguer::Confirm::new()
                .with_prompt(prompt)
                .default(false)
                .show_default(true)
                .interact()
                .unwrap_or(false);
            if !proceed {
                return Ok(InstallOutcome::Skipped(target));
            }
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, bytes)?;
        Ok(InstallOutcome::Written(target))
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
            Some(Commands::Completion { cargo_fresh, install, .. }) => {
                assert!(cargo_fresh);
                assert!(!install);
            }
            _ => panic!("expected completion subcommand"),
        }
    }

    #[test]
    fn cli_completion_install_flag() {
        let cli = Cli::try_parse_from(["cargo-fresh", "completion", "fish", "--install"]).expect("parse");
        match cli.command {
            Some(Commands::Completion { install, cargo_fresh, .. }) => {
                assert!(install);
                assert!(!cargo_fresh);
            }
            _ => panic!("expected completion subcommand"),
        }
    }

    #[test]
    fn install_target_path_fish_top_level() {
        let path = Cli::install_target_path(&ShellType::Fish, false).expect("fish supported");
        assert!(path.ends_with("fish/completions/cargo-fresh.fish"));
    }

    #[test]
    fn install_target_path_fish_cargo_subcommand() {
        let path = Cli::install_target_path(&ShellType::Fish, true).expect("fish supported");
        assert!(path.ends_with("fish/completions/cargo.fish"));
    }

    #[test]
    fn install_target_path_unsupported_shell_returns_none() {
        assert!(Cli::install_target_path(&ShellType::Bash, false).is_none());
        assert!(Cli::install_target_path(&ShellType::Zsh, true).is_none());
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

    #[test]
    fn parses_check_prebuilt() {
        let cli = Cli::try_parse_from(["cargo-fresh", "--check-prebuilt"]).unwrap();
        assert!(cli.check_prebuilt);
    }
}
