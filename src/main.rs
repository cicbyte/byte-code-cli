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
    let mut cfg = match config::load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("bcode: {e:#}");
            std::process::exit(2);
        }
    };
    // 命令行 > 配置文件：--insecure 旗标覆盖 config.toml
    if cli.insecure {
        cfg.insecure = true;
    }
    if cfg.insecure {
        eprintln!("bcode: 警告：TLS 证书校验已关闭（--insecure），仅限自签/调试环境使用");
    }
    let profile = config::effective_profile(&cfg, cli.profile.as_ref());

    let result = match cli.command {
        Command::Register {
            ref name,
            ref capabilities,
        } => cmd::identity::register(&cfg, &profile, name, capabilities.as_deref(), &out).await,
        Command::Whoami => cmd::identity::whoami(&cfg, &profile, &out).await,
        Command::Status => cmd::project::status(&cfg, &profile, &out).await,
        Command::Profiles => cmd::identity::profiles(&cfg, &out),
        Command::Join { ref code } => cmd::project::join(&cfg, &profile, code, &out).await,
        Command::Start { ref project } => {
            cmd::project::start(&cfg, &profile, project.as_deref(), &out).await
        }
        Command::Context => cmd::project::context(&cfg, &profile, &out).await,
        Command::Tasks {
            ref status,
            ref keyword,
        } => cmd::tasks::tasks(&cfg, &profile, status.as_deref(), keyword.as_deref(), &out).await,
        Command::Task { id } => cmd::tasks::task(&cfg, &profile, id, &out).await,
        Command::Claim { id } => cmd::tasks::claim(&cfg, &profile, id, &out).await,
        Command::Complete {
            id,
            ref artifacts,
            ref artifacts_file,
            ref note,
        } => {
            cmd::tasks::complete(
                &cfg,
                &profile,
                id,
                artifacts.as_deref(),
                artifacts_file.as_deref(),
                note.as_deref(),
                &out,
            )
            .await
        }
        Command::Log {
            id,
            ref message,
            ref status,
            ref action,
        } => cmd::tasks::log(&cfg, &profile, id, message, status, action, &out).await,
        Command::Comment { task_id, ref text } => {
            cmd::comms::comment(&cfg, &profile, task_id, text, &out).await
        }
        Command::Comments {
            task_id,
            follow,
            interval,
        } => cmd::comms::comments(&cfg, &profile, task_id, follow, interval, &out).await,
        Command::Notify { unread, watch } => {
            cmd::comms::notify(&cfg, &profile, unread, watch, &out).await
        }
        Command::Docs { ref path, list } => {
            cmd::vault::docs(&cfg, &profile, path.as_deref(), list, &out).await
        }
        Command::Memory { ref key } => cmd::vault::memory(&cfg, &profile, key, &out).await,
        Command::Memories { ref prefix } => {
            cmd::vault::memories(&cfg, &profile, prefix.as_deref(), &out).await
        }
        Command::Search {
            ref query,
            ref module,
        } => cmd::vault::search(&cfg, &profile, query, module.as_deref(), &out).await,
    };

    if let Err(e) = result {
        let code = error::exit_code_of(&e);
        eprintln!("bcode: {e:#}");
        std::process::exit(code as i32);
    }
}
