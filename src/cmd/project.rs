//! 项目域命令（准入与会话层）：status 在线校验。

use anyhow::{Result, bail};
use serde_json::json;

use crate::client::BcodeClient;
use crate::config::{self, Config};
use crate::cred;
use crate::model;
use crate::output::Out;

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
    // 未建立会话是正常态（M0 无 start 命令），降级提示而非报错
    let Some(sess) = cred::load_session(profile, ptr.project_id) else {
        out.kv("连接", "凭证有效，但未建立会话（bcode start）");
        payload["connected"] = json!(false);
        payload["reason"] = json!("no_session");
        out.emit_value(&payload);
        return Ok(());
    };

    let client = BcodeClient::new(server, credential, Some(sess.session_id))?;
    let tasks = client
        .get_as::<model::TaskList>("/v1/agent/tasks?size=5")
        .await?;

    out.kv("连接", "正常（身份/准入/会话全部有效）");
    out.kv("可见任务", &format!("{} 条", tasks.total));
    payload["connected"] = json!(true);
    payload["tasks_total"] = json!(tasks.total);

    out.emit_value(&payload);
    Ok(())
}
