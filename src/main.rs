//! bcode — ByteCode CLI：外部 coding agent 与 ByteCode 平台之间的本地桥。
//! 薄入口：解析 → 分发 → 退出码；全部逻辑在 lib crate（MCP 化时复用）。
//! 协议：dev-docs/agent-protocol.md v2（三层模型：注册/准入/会话）。

use clap::Parser;

use bcode::cli::{Cli, Command};
use bcode::cmd;
use bcode::output::Out;
use bcode::{config, error};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let out = Out { json: cli.json };

    // 环境错误（读/解析 config.toml 失败）与用法错同走退出码 2，不 panic
    let cfg = match config::load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("bcode: {e:#}");
            std::process::exit(2);
        }
    };
    let profile = config::effective_profile(&cfg, cli.profile.as_ref());

    let result = match cli.command {
        Command::Whoami => cmd::identity::whoami(&cfg, &profile, &out).await,
        Command::Status => cmd::project::status(&cfg, &profile, &out).await,
        Command::Profiles => cmd::identity::profiles(&cfg, &out),
    };

    if let Err(e) = result {
        let code = error::exit_code_of(&e);
        eprintln!("bcode: {e:#}");
        std::process::exit(code as i32);
    }
}
