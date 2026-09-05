//! 项目域命令（准入与会话层）：join 接入 / start 会话+开工包 /
//! context 约定输出 / status 在线校验。

use anyhow::{Result, anyhow, bail};
use serde_json::json;

use super::ensure_session;
use crate::client::BcodeClient;
use crate::config::{self, Config};
use crate::cred::{self, ProjectPointer};
use crate::model::agent::{AgentTasks, JoinResult, SessionCreated};
use crate::model::platform::ProjectList;
use crate::output::Out;

/// `bcode join <code>`（F03）：接入码换项目准入，成功写 <repo>/.bc/project。
pub async fn join(cfg: &Config, profile: &str, code: &str, out: &Out) -> Result<()> {
    let client = super::identity_client(cfg, profile)?;
    let res: JoinResult = client
        .post_as("/v1/agent/projects/join", json!({ "code": code }))
        .await?;

    let ptr = ProjectPointer {
        project_id: res.project_id,
        project_name: res.project_name.clone(),
    };
    let cwd = std::env::current_dir()?;
    let marker = config::save_project_pointer(&cwd, &ptr)?;

    out.kv(
        "project",
        &format!("{} (id={})", res.project_name, res.project_id),
    );
    out.kv("写入", &marker.display().to_string());
    out.line("（下一步：bcode start 建立会话并查看开工包）");

    out.emit_value(&json!({
        "project": { "id": ptr.project_id, "name": ptr.project_name },
        "pointer_path": marker.display().to_string(),
    }));
    Ok(())
}

/// `bcode start [--project <id|名>]`（F04）：建立/续期会话并展示开工包。
pub async fn start(
    cfg: &Config,
    profile: &str,
    project_flag: Option<&str>,
    out: &Out,
) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;

    // 项目解析：--project（数字 id 或名称）优先，缺省取 .bc/project 指向
    let project_id = match project_flag {
        Some(spec) => {
            let client = BcodeClient::new(server.clone(), credential.clone(), None)?;
            resolve_project(&client, spec).await?
        }
        None => {
            let cwd = std::env::current_dir()?;
            config::find_project_pointer(&cwd)?
                .ok_or_else(|| {
                    anyhow!("当前目录不在任何项目内：先 bcode join，或用 --project <id|名> 指定")
                })?
                .project_id
        }
    };

    let boot = ensure_session(&server, profile, &credential, project_id).await?;
    display_kickoff(&boot, out);
    out.emit_value(&serde_json::to_value(&boot)?);
    Ok(())
}

/// `bcode context`（F14）：开工包约定（conventions）原样输出——
/// AI 建立项目认知的入口。会话同键复用续期，此处重取保证约定最新。
pub async fn context(cfg: &Config, profile: &str, out: &Out) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    let cwd = std::env::current_dir()?;
    let ptr = config::find_project_pointer(&cwd)?.ok_or_else(|| {
        anyhow!("当前目录不在任何项目内：在项目仓库根目录执行，或先 bcode join <接入码>")
    })?;

    let boot = ensure_session(&server, profile, &credential, ptr.project_id).await?;
    if boot.conventions.is_empty() {
        out.line("（无约定——项目与全局记忆均未配置 conventions.*）");
    }
    for c in &boot.conventions {
        out.line(&format!("[{}] {} = {}", c.scope, c.key, c.value));
    }
    out.emit_value(&json!({ "conventions": serde_json::to_value(&boot.conventions)? }));
    Ok(())
}

/// `bcode status`：打一次网络校验身份与会话的真实有效性（whoami 的在线版）。
/// 无凭证/无项目指向时给出分步引导而不是报错堆栈。
pub async fn status(cfg: &Config, profile: &str, out: &Out) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    let agent_name = credential.name.clone();

    let cwd = std::env::current_dir()?;
    let Some(ptr) = config::find_project_pointer(&cwd)? else {
        bail!("当前目录不在任何项目内：在项目仓库根目录执行，或先 bcode join <接入码>");
    };

    let mut payload = json!({
        "agent": agent_name,
        "project": { "id": ptr.project_id, "name": ptr.project_name },
    });

    out.kv("agent", &agent_name);
    out.kv(
        "project",
        &format!("{} (id={})", ptr.project_name, ptr.project_id),
    );

    // 会话存在才带 X-Session 调免参任务端点（一举校验 key+准入+会话三件）；
    // 未建立会话是正常态，降级提示而非报错
    let Some(sess) = cred::load_session(profile, ptr.project_id) else {
        out.kv("连接", "凭证有效，但未建立会话（bcode start）");
        payload["connected"] = json!(false);
        payload["reason"] = json!("no_session");
        out.emit_value(&payload);
        return Ok(());
    };

    let client = BcodeClient::new(server, credential, Some(sess.session_id))?;
    let tasks = client.get_as::<AgentTasks>("/v1/agent/tasks").await?;

    out.kv("连接", "正常（身份/准入/会话全部有效）");
    out.kv("可见任务", &format!("{} 条", tasks.total));
    payload["connected"] = json!(true);
    payload["tasks_total"] = json!(tasks.total);

    out.emit_value(&payload);
    Ok(())
}

/// --project 解析：数字直接当 id；名称在已加入项目里匹配
/// （agent 视角 GET /v1/projects 只含已获准入的项目）
async fn resolve_project(client: &BcodeClient, spec: &str) -> Result<i64> {
    if let Ok(id) = spec.parse::<i64>() {
        return Ok(id);
    }
    let list: ProjectList = client.get_as("/v1/projects?size=100").await?;
    let hit = list.list.iter().find(|p| p.name == spec).ok_or_else(|| {
        anyhow!("未找到名为「{spec}」的项目（agent 只能看到已加入的项目；也可直接用数字 id）")
    })?;
    Ok(hit.id)
}

/// 开工包三段展示：约定 / 我的任务 / 待审
fn display_kickoff(boot: &SessionCreated, out: &Out) {
    out.kv("session", &boot.session_id);
    out.kv(
        "project",
        &format!("{} (id={})", boot.project.name, boot.project.id),
    );
    out.line("");
    out.line(&format!("── 约定（{} 条）──", boot.conventions.len()));
    if boot.conventions.is_empty() {
        out.line("  （无）");
    }
    for c in &boot.conventions {
        out.line(&format!("  [{}] {} = {}", c.scope, c.key, c.value));
    }
    out.line("");
    out.line(&format!("── 我的任务（{}）──", boot.my_tasks.len()));
    if boot.my_tasks.is_empty() {
        out.line("  （无）");
    }
    for t in &boot.my_tasks {
        out.line(&format!("  {}", t.one_line()));
    }
    out.line("");
    out.line(&format!("── 待审（{}）──", boot.pending_reviews.len()));
    if boot.pending_reviews.is_empty() {
        out.line("  （无）");
    }
    for t in &boot.pending_reviews {
        out.line(&format!("  {}", t.one_line()));
    }
}
