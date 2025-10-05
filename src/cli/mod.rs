use clap::{CommandFactory, Parser};

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

    /// 生成 shell 补全脚本
    #[arg(long, value_name = "SHELL")]
    pub completion: Option<String>,
}

impl Cli {
    pub fn generate_completion(shell: String) {
        let mut cmd = Cli::command();
        let shell = shell.to_lowercase();

        let shell_type = match shell.as_str() {
            "bash" => clap_complete::Shell::Bash,
            "zsh" => clap_complete::Shell::Zsh,
            "fish" => clap_complete::Shell::Fish,
            "powershell" => clap_complete::Shell::PowerShell,
            "elvish" => clap_complete::Shell::Elvish,
            _ => {
                eprintln!(
                    "不支持的 shell: {}. 支持的 shell: bash, zsh, fish, powershell, elvish",
                    shell
                );
                std::process::exit(1);
            }
        };

        clap_complete::generate(shell_type, &mut cmd, "pkg-checker", &mut std::io::stdout());
    }
}
