//! 命令实现，按协议域分组：identity（注册/身份）、project（准入与会话）、
//! tasks（任务工作流）、comms（通信）、vault（文档/记忆/搜索 上下文消费）、
//! system（init/open/completion/man 便利命令）。

pub mod comms;
pub mod identity;
pub mod project;
pub mod system;
pub mod tasks;
pub mod vault;

use anyhow::{Result, anyhow};
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::client::BcodeClient;
use crate::config::{self, Config};
use crate::cred::{self, Credential, ProjectPointer};
use crate::error::BcodeError;
use crate::model::agent::SessionCreated;

/// 项目作用域上下文：--profile 身份 + cwd 项目指向 + 会话（免参命令共用）
pub(crate) struct Ctx {
    pub client: BcodeClient,
    pub credential: Credential,
    pub project: ProjectPointer,
    server: String,
    profile: String,
    insecure: bool,
}

impl Ctx {
    /// 会话免参 GET：缓存会话失效时重建后重试一次。触发面：
    /// Auth 错（key 对但会话被平台废弃）或「会话不存在」业务错
    /// （本地缓存属于旧 agent——换 key 重注册后 id 变化）。
    /// 二次仍失败按原错误透出（key/准入问题，引导 register/join）
    pub(crate) async fn sessioned_get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        match self.client.get_as::<T>(path).await {
            Ok(v) => Ok(v),
            Err(e) => {
                if !is_auth(&e) && !is_stale_session(&e) {
                    return Err(e);
                }
                // 会话键=agent+project 服务端同键复用，重建幂等
                let boot = ensure_session(
                    &self.server,
                    &self.profile,
                    &self.credential,
                    self.project.project_id,
                    self.insecure,
                )
                .await?;
                let fresh = BcodeClient::new(
                    self.server.clone(),
                    self.credential.clone(),
                    Some(boot.session_id),
                    self.insecure,
                )?;
                fresh.get_as::<T>(path).await
            }
        }
    }
}

fn is_auth(e: &anyhow::Error) -> bool {
    matches!(e.downcast_ref::<BcodeError>(), Some(BcodeError::Auth(_)))
}

/// 「会话不存在」：平台对无效 X-Session 返回的业务错（实测：换 key 重注册后
/// 本地缓存 session 属旧 agent_id，平台以 Business 而非 Auth 报出）
fn is_stale_session(e: &anyhow::Error) -> bool {
    matches!(
        e.downcast_ref::<BcodeError>(),
        Some(BcodeError::Business(m)) if m.contains("会话")
    )
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
    let client = BcodeClient::new(
        server.clone(),
        credential.clone(),
        Some(session_id),
        cfg.insecure,
    )?;
    Ok(Ctx {
        client,
        credential,
        project,
        server,
        profile: profile.to_string(),
        insecure: cfg.insecure,
    })
}

/// 纯身份上下文（无项目/会话）：join / notify / search 用
pub(crate) fn identity_client(cfg: &Config, profile: &str) -> Result<BcodeClient> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    BcodeClient::new(server, credential, None, cfg.insecure)
}
