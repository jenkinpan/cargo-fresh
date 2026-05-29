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
        /// Generate completion for the `cargo fresh` subcommand form instead of
        /// the top-level `cargo-fresh` binary. Ignored when `--install` is set
        /// (the interactive prompt covers both targets).
        #[arg(long)]
        cargo_fresh: bool,
        /// Install the completion script to its standard location for the chosen
        /// shell. Interactive: prompts which target(s) to install (the top-level
        /// `cargo-fresh<TAB>` completion, the `cargo fresh<TAB>` completion, or
        /// both). Use `--install --yes` (or pipe stdin) to skip the prompt and
        /// install both.
        #[arg(long)]
        install: bool,
        /// Skip the interactive picker and install all targets non-interactively.
        /// Only meaningful with `--install`.
        #[arg(long)]
        yes: bool,
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

/// 单次 `--install` 写文件的结果——调用者据此决定打 `Installed` 还是 `Skip` 状态行。
pub enum InstallOutcome {
    Written(std::path::PathBuf),
    Skipped(std::path::PathBuf),
}

/// `--install` 的两种目标：顶层 `cargo-fresh<TAB>`，或 cargo 子命令 `cargo fresh<TAB>`。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InstallTarget {
    /// 给独立二进制用的补全（`cargo-fresh<TAB>` 会触发）。
    TopLevel,
    /// 给 cargo 子命令形式用的补全（`cargo fresh<TAB>` 会触发）。
    CargoSubcommand,
}

impl InstallTarget {
    pub fn is_cargo_subcommand(self) -> bool {
        matches!(self, InstallTarget::CargoSubcommand)
    }
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

impl ShellType {
    fn display_name(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::Powershell => "powershell",
            ShellType::Elvish => "elvish",
            ShellType::Nushell => "nushell",
        }
    }
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
    fn render_completion_to_bytes(shell: &ShellType, target: InstallTarget) -> Vec<u8> {
        let mut buf = Vec::new();
        if target.is_cargo_subcommand() {
            let mut cargo_cmd = Self::build_cargo_fresh_command();
            Self::render_completion_into(shell, &mut cargo_cmd, "cargo", &mut buf);
        } else {
            let mut cmd = Self::command();
            Self::render_completion_into(shell, &mut cmd, "cargo-fresh", &mut buf);
        }
        buf
    }

    fn config_home() -> Option<std::path::PathBuf> {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
    }

    fn data_home() -> Option<std::path::PathBuf> {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share")))
    }

    fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
    }

    /// 计算 `--install` 写入路径。覆盖所有受支持的 shell；其中 zsh/powershell/elvish/nushell
    /// 写入位置不会被 shell 默认 auto-load，所以 [`install_post_hint`] 会回一个补充提示
    /// （加入 fpath、`. $PROFILE` 等），调用者把它打到 stderr。
    pub fn install_target_path(
        shell: &ShellType,
        target: InstallTarget,
    ) -> Option<std::path::PathBuf> {
        let is_cargo = target.is_cargo_subcommand();
        match shell {
            ShellType::Fish => {
                let base = Self::config_home()?;
                let file = if is_cargo { "cargo.fish" } else { "cargo-fresh.fish" };
                Some(base.join("fish").join("completions").join(file))
            }
            ShellType::Bash => {
                let base = Self::data_home()?;
                // bash-completion 自动从 $XDG_DATA_HOME/bash-completion/completions/ 加载
                // 同名于命令的文件 — 文件名不要 .bash 后缀。
                let file = if is_cargo { "cargo" } else { "cargo-fresh" };
                Some(base.join("bash-completion").join("completions").join(file))
            }
            ShellType::Zsh => {
                // 没有跨发行版统一的 fpath 第一个目录，写到 ~/.zfunc，并在 hint 里教用户挂 fpath。
                let base = Self::home_dir()?;
                let file = if is_cargo { "_cargo" } else { "_cargo-fresh" };
                Some(base.join(".zfunc").join(file))
            }
            ShellType::Nushell => {
                let base = Self::config_home()?;
                let file = if is_cargo { "cargo.nu" } else { "cargo-fresh.nu" };
                Some(base.join("nushell").join("completions").join(file))
            }
            ShellType::Elvish => {
                let base = Self::config_home()?;
                let file = if is_cargo { "cargo.elv" } else { "cargo-fresh.elv" };
                Some(base.join("elvish").join("lib").join(file))
            }
            ShellType::Powershell => {
                let base = Self::config_home()?;
                let file = if is_cargo { "cargo.ps1" } else { "cargo-fresh.ps1" };
                Some(base.join("powershell").join(file))
            }
        }
    }

    /// 写好补全文件之后给用户的一行 “还要做这步才能生效” 提示。返回 None 表示
    /// 这个 shell 默认会自动加载，不需要额外步骤（fish / bash-completion 走这条）。
    pub fn install_post_hint(
        shell: &ShellType,
        target: InstallTarget,
        path: &std::path::Path,
    ) -> Option<String> {
        let path_str = path.display();
        match shell {
            ShellType::Bash | ShellType::Fish => None,
            ShellType::Zsh => Some(
                "make sure ~/.zfunc is on $fpath: add `fpath=(~/.zfunc $fpath)` then `autoload -Uz compinit && compinit` to ~/.zshrc"
                    .to_string(),
            ),
            ShellType::Powershell => Some(format!(
                "dot-source from $PROFILE: add `. \"{path_str}\"` to your $PROFILE"
            )),
            ShellType::Elvish => {
                let module = if target.is_cargo_subcommand() { "cargo" } else { "cargo-fresh" };
                Some(format!("add `use {module}` to ~/.config/elvish/rc.elv"))
            }
            ShellType::Nushell => Some(format!(
                "add `source \"{path_str}\"` to your config.nu (or `$nu.config-path`)"
            )),
        }
    }

    /// 把单个目标的补全脚本写到 [`install_target_path`] 给出的位置。已存在文件时通过
    /// dialoguer 询问是否覆盖；非 TTY / 用户拒绝时返回 [`InstallOutcome::Skipped`] 让调
    /// 用者打 skip 状态。`force` 跳过覆盖确认（用于 `--yes` 路径）。
    pub fn install_completion_target(
        shell: &ShellType,
        target: InstallTarget,
        language: crate::locale::Language,
        force: bool,
    ) -> anyhow::Result<InstallOutcome> {
        let path = Self::install_target_path(shell, target).ok_or_else(|| {
            anyhow::anyhow!(
                "{}",
                language.format_text(
                    "completion_install_no_home",
                    &[("shell", shell.display_name())],
                )
            )
        })?;

        let bytes = Self::render_completion_to_bytes(shell, target);

        if path.exists() && !force {
            if !std::io::stderr().is_terminal() {
                return Ok(InstallOutcome::Skipped(path));
            }
            let prompt = language
                .get_text("completion_overwrite_prompt")
                .replace("{}", &path.display().to_string());
            let proceed = dialoguer::Confirm::new()
                .with_prompt(prompt)
                .default(false)
                .show_default(true)
                .interact()
                .unwrap_or(false);
            if !proceed {
                return Ok(InstallOutcome::Skipped(path));
            }
        }

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes)?;
        Ok(InstallOutcome::Written(path))
    }

    /// 用 dialoguer 的 MultiSelect 问用户要装哪些目标。两项均默认勾选；
    /// 非 TTY / `yes == true` 时跳过提示，直接返回两个都选。
    pub fn select_install_targets(
        language: crate::locale::Language,
        yes: bool,
    ) -> anyhow::Result<Vec<InstallTarget>> {
        let all = vec![InstallTarget::TopLevel, InstallTarget::CargoSubcommand];

        if yes || !std::io::stderr().is_terminal() {
            return Ok(all);
        }

        let items = [
            language.get_text("completion_target_top"),
            language.get_text("completion_target_cargo"),
        ];

        let chosen = dialoguer::MultiSelect::new()
            .with_prompt(language.get_text("completion_install_prompt"))
            .items(items)
            .defaults(&[true, true])
            .interact()?;

        let selected: Vec<InstallTarget> = chosen
            .into_iter()
            .filter_map(|idx| match idx {
                0 => Some(InstallTarget::TopLevel),
                1 => Some(InstallTarget::CargoSubcommand),
                _ => None,
            })
            .collect();
        Ok(selected)
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
            Some(Commands::Completion { cargo_fresh, install, yes, .. }) => {
                assert!(cargo_fresh);
                assert!(!install);
                assert!(!yes);
            }
            _ => panic!("expected completion subcommand"),
        }
    }

    #[test]
    fn cli_completion_install_flag() {
        let cli = Cli::try_parse_from(["cargo-fresh", "completion", "fish", "--install"]).expect("parse");
        match cli.command {
            Some(Commands::Completion { install, cargo_fresh, yes, .. }) => {
                assert!(install);
                assert!(!cargo_fresh);
                assert!(!yes);
            }
            _ => panic!("expected completion subcommand"),
        }
    }

    #[test]
    fn cli_completion_install_yes_flag() {
        let cli = Cli::try_parse_from(["cargo-fresh", "completion", "fish", "--install", "--yes"])
            .expect("parse");
        match cli.command {
            Some(Commands::Completion { install, yes, .. }) => {
                assert!(install);
                assert!(yes);
            }
            _ => panic!("expected completion subcommand"),
        }
    }

    #[test]
    fn install_target_path_fish_top_level() {
        let path = Cli::install_target_path(&ShellType::Fish, InstallTarget::TopLevel)
            .expect("fish supported");
        assert!(path.ends_with("fish/completions/cargo-fresh.fish"));
    }

    #[test]
    fn install_target_path_fish_cargo_subcommand() {
        let path = Cli::install_target_path(&ShellType::Fish, InstallTarget::CargoSubcommand)
            .expect("fish supported");
        assert!(path.ends_with("fish/completions/cargo.fish"));
    }

    #[test]
    fn install_target_path_bash_paths() {
        let top = Cli::install_target_path(&ShellType::Bash, InstallTarget::TopLevel).unwrap();
        assert!(top.ends_with("bash-completion/completions/cargo-fresh"));
        let cargo = Cli::install_target_path(&ShellType::Bash, InstallTarget::CargoSubcommand).unwrap();
        assert!(cargo.ends_with("bash-completion/completions/cargo"));
    }

    #[test]
    fn install_target_path_zsh_paths() {
        let top = Cli::install_target_path(&ShellType::Zsh, InstallTarget::TopLevel).unwrap();
        assert!(top.ends_with(".zfunc/_cargo-fresh"));
        let cargo = Cli::install_target_path(&ShellType::Zsh, InstallTarget::CargoSubcommand).unwrap();
        assert!(cargo.ends_with(".zfunc/_cargo"));
    }

    #[test]
    fn install_target_path_nushell_paths() {
        let top = Cli::install_target_path(&ShellType::Nushell, InstallTarget::TopLevel).unwrap();
        assert!(top.ends_with("nushell/completions/cargo-fresh.nu"));
    }

    #[test]
    fn install_target_path_elvish_powershell_paths() {
        let elv = Cli::install_target_path(&ShellType::Elvish, InstallTarget::TopLevel).unwrap();
        assert!(elv.ends_with("elvish/lib/cargo-fresh.elv"));
        let ps = Cli::install_target_path(&ShellType::Powershell, InstallTarget::CargoSubcommand).unwrap();
        assert!(ps.ends_with("powershell/cargo.ps1"));
    }

    #[test]
    fn install_post_hint_fish_and_bash_none() {
        let path = std::path::PathBuf::from("/tmp/x");
        assert!(Cli::install_post_hint(&ShellType::Fish, InstallTarget::TopLevel, &path).is_none());
        assert!(Cli::install_post_hint(&ShellType::Bash, InstallTarget::TopLevel, &path).is_none());
    }

    #[test]
    fn install_post_hint_zsh_mentions_fpath() {
        let path = std::path::PathBuf::from("/tmp/_cargo-fresh");
        let hint = Cli::install_post_hint(&ShellType::Zsh, InstallTarget::TopLevel, &path).unwrap();
        assert!(hint.contains("fpath"));
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
