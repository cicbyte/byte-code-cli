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
    // profile 拼进本地路径，非法名（路径分隔符等）在入口拦截，退出码 2
    if let Err(e) = config::validate_profile_name(&profile) {
        eprintln!("bcode: {e:#}");
        std::process::exit(2);
    }

    let result = match cli.command {
        Command::Register(a) => {
            cmd::identity::register(&cfg, &profile, &a.name, a.capabilities.as_deref(), &out).await
        }
        Command::Whoami => cmd::identity::whoami(&cfg, &profile, &out).await,
        Command::Status => cmd::project::status(&cfg, &profile, &out).await,
        Command::Profiles => cmd::identity::profiles(&cfg, &out),
        Command::Join(a) => cmd::project::join(&cfg, &profile, &a.code, &out).await,
        Command::Projects => cmd::project::projects(&cfg, &profile, &out).await,
        Command::Start(a) => cmd::project::start(&cfg, &profile, a.project.as_deref(), &out).await,
        Command::Context => cmd::project::context(&cfg, &profile, &out).await,
        Command::Tasks(a) => {
            cmd::tasks::tasks(
                &cfg,
                &profile,
                a.status.as_deref(),
                a.keyword.as_deref(),
                a.priority,
                &a.sort,
                &out,
            )
            .await
        }
        Command::Task(a) => cmd::tasks::task(&cfg, &profile, a.id, &out).await,
        Command::Create(a) => cmd::tasks::create(&cfg, &profile, &a, &out).await,
        Command::Update(a) => cmd::tasks::update(&cfg, &profile, &a, &out).await,
        Command::Claim(a) => cmd::tasks::claim(&cfg, &profile, a.id, &out).await,
        Command::Complete(a) => {
            cmd::tasks::complete(
                &cfg,
                &profile,
                a.id,
                a.artifacts.as_deref(),
                a.artifacts_file.as_deref(),
                a.note.as_deref(),
                &out,
            )
            .await
        }
        Command::Log(a) => {
            cmd::tasks::log(&cfg, &profile, a.id, &a.message, &a.status, &a.action, &out).await
        }
        Command::Comment(a) => cmd::comms::comment(&cfg, &profile, a.task_id, &a.text, &out).await,
        Command::Comments(a) => {
            cmd::comms::comments(&cfg, &profile, a.task_id, a.follow, a.interval, &out).await
        }
        Command::Notify(a) => cmd::comms::notify(&cfg, &profile, &a, &out).await,
        Command::Docs(a) => {
            cmd::vault::docs(
                &cfg,
                &profile,
                a.path.as_deref(),
                a.list,
                a.write_file.as_deref(),
                &out,
            )
            .await
        }
        Command::Memory(a) => cmd::vault::memory(&cfg, &profile, &a, &out).await,
        Command::Memories(a) => {
            cmd::vault::memories(&cfg, &profile, a.prefix.as_deref(), &out).await
        }
        Command::Search(a) => {
            cmd::vault::search(&cfg, &profile, &a.query, a.module.as_deref(), &out).await
        }
        Command::Init(a) => cmd::system::init(&mut cfg, a.url.as_deref(), &out).await,
        Command::Open(a) => cmd::system::open(&cfg, &a.target, a.id, &out),
        Command::Completion(a) => cmd::system::completion(a.shell),
        Command::Man => cmd::system::man(),
    };

    if let Err(e) = result {
        let code = error::exit_code_of(&e);
        eprintln!("bcode: {e:#}");
        std::process::exit(code as i32);
    }
}
