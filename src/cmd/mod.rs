//! 命令实现，按协议域分组：identity（注册/身份）、project（准入与会话）、
//! tasks（任务工作流）、comms（通信）、vault（文档/记忆/搜索 上下文消费）。

pub mod comms;
pub mod identity;
pub mod project;
pub mod tasks;
pub mod vault;

use anyhow::{Result, anyhow};
use serde_json::json;

use crate::client::BcodeClient;
use crate::config::{self, Config};
use crate::cred::{self, Credential, ProjectPointer};
use crate::model::agent::SessionCreated;

/// 项目作用域上下文：--profile 身份 + cwd 项目指向 + 会话（免参命令共用）
pub(crate) struct Ctx {
    pub client: BcodeClient,
    pub credential: Credential,
    pub project: ProjectPointer,
}

/// 建立/续期工作会话并落盘（start 显式调用；其余命令缺会话时懒建立，F04）
pub(crate) async fn ensure_session(
    server: &str,
    profile: &str,
    credential: &Credential,
    project_id: i64,
    insecure: bool,
) -> Result<SessionCreated> {
    let client = BcodeClient::new(server.to_string(), credential.clone(), None, insecure)?;
    let boot = client
        .post_as::<SessionCreated>("/v1/agent/sessions", json!({ "projectId": project_id }))
        .await?;
    // 会话记录以服务端返回为准（--project 传纯 id 时本地无名称）
    cred::save_session(
        profile,
        &cred::SessionRecord {
            session_id: boot.session_id.clone(),
            project_id: boot.project.id,
            project_name: boot.project.name.clone(),
        },
    )?;
    Ok(boot)
}

/// 项目作用域上下文：凭证 + .bc/project 指向 + 会话（缺则静默 start）
pub(crate) async fn project_ctx(cfg: &Config, profile: &str) -> Result<Ctx> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    let cwd = std::env::current_dir()?;
    let project = config::find_project_pointer(&cwd)?.ok_or_else(|| {
        anyhow!("当前目录不在任何项目内：在项目仓库根目录执行，或先 bcode join <接入码>")
    })?;

    let session_id = match cred::load_session(profile, project.project_id) {
        Some(s) => s.session_id,
        None => {
            ensure_session(
                &server,
                profile,
                &credential,
                project.project_id,
                cfg.insecure,
            )
            .await?
            .session_id
        }
    };
    let client = BcodeClient::new(server, credential.clone(), Some(session_id), cfg.insecure)?;
    Ok(Ctx {
        client,
        credential,
        project,
    })
}

/// 纯身份上下文（无项目/会话）：join / notify / search 用
pub(crate) fn identity_client(cfg: &Config, profile: &str) -> Result<BcodeClient> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    BcodeClient::new(server, credential, None, cfg.insecure)
}
