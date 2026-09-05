//! 任务域命令（工作会话层）：tasks 列表 / task 详情 /
//! claim 认领 / complete 完成 / log 过程留痕（F06-F10）。

use anyhow::{Context, Result, bail};
use serde_json::json;

use super::project_ctx;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::agent::AgentTasks;
use crate::model::task::{AiLogList, CommentList, CreatedId, TaskDetail};
use crate::output::Out;

/// `bcode tasks [--status s] [--keyword kw]`（F06）：免参任务列表。
/// status 透传平台语义：缺省=未完成三态（open/in_progress/review），all=全部。
pub async fn tasks(
    cfg: &Config,
    profile: &str,
    status: Option<&str>,
    keyword: Option<&str>,
    out: &Out,
) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;

    let mut path = String::from("/v1/agent/tasks");
    let mut sep = '?';
    if let Some(s) = status {
        path.push(sep);
        path.push_str(&format!("status={}", encode_query(s)));
        sep = '&';
    }
    if let Some(kw) = keyword {
        path.push(sep);
        path.push_str(&format!("keyword={}", encode_query(kw)));
    }
    let page: AgentTasks = ctx.client.get_as(&path).await?;

    if page.list.is_empty() {
        out.line(&format!("（无任务，共 {} 条记录）", page.total));
    } else {
        out.line(&format!(
            "  {:<7} {:<13} {:<4} {:<12} {}",
            "id", "状态", "优先", "截止", "标题"
        ));
        for t in &page.list {
            let due = t.due_date.as_deref().unwrap_or("—");
            out.line(&format!(
                "  {:<7} {:<13} {:<4} {:<12} {}",
                t.id,
                t.status,
                format!("P{}", t.priority),
                due,
                t.title
            ));
        }
        out.line(&format!("（{} 条）", page.total));
    }
    out.emit_value(&serde_json::to_value(&page)?);
    Ok(())
}

/// `bcode task <id>`（F10）：单任务详情——描述 / artifacts / 执行日志 / 评论摘要。
pub async fn task(cfg: &Config, profile: &str, id: i64, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let detail: TaskDetail = ctx.client.get_as(&format!("/v1/tasks/{id}")).await?;
    let logs: AiLogList = ctx
        .client
        .get_as(&format!("/v1/tasks/{id}/ai-logs"))
        .await?;
    let comments: CommentList = ctx
        .client
        .get_as(&format!("/v1/tasks/{id}/comments"))
        .await?;

    out.line("");
    out.line(&format!(
        "#{} {}  [{}] P{}",
        detail.id, detail.title, detail.status, detail.priority
    ));
    out.kv("类型", &detail.r#type);
    out.kv(
        "认领人",
        if detail.assignee_name.is_empty() {
            "—"
        } else {
            &detail.assignee_name
        },
    );
    out.kv("创建人", &detail.creator_name);
    out.kv(
        "截止",
        if detail.due_date.is_empty() {
            "—"
        } else {
            &detail.due_date
        },
    );
    if !detail.tags.is_empty() {
        out.kv("标签", &detail.tags.join(", "));
    }
    if !detail.description.is_empty() {
        out.line("");
        out.line("── 描述 ──");
        out.line(&detail.description);
    }
    if !detail.artifacts.is_empty() {
        out.line("");
        out.line("── Artifacts ──");
        out.line(&detail.artifacts);
    }
    out.line("");
    out.line(&format!(
        "── 执行日志（最近 {} / 共 {}）──",
        logs.list.len().min(5),
        logs.list.len()
    ));
    for l in logs.list.iter().rev().take(5).rev() {
        out.line(&format!(
            "  [{}] {} · {} ({})",
            l.created_at, l.ai_username, l.action, l.status
        ));
        if !l.detail.is_empty() {
            out.line(&format!("    {}", l.detail));
        }
    }
    out.line("");
    out.line(&format!(
        "── 评论（最近 {} / 共 {}）──",
        comments.list.len().min(5),
        comments.list.len()
    ));
    for c in comments.list.iter().rev().take(5).rev() {
        out.line(&format!(
            "  [{}] {}: {}",
            c.created_at, c.real_name, c.content
        ));
    }

    out.emit_value(&json!({
        "task": serde_json::to_value(&detail)?,
        "logs": serde_json::to_value(&logs)?,
        "comments": serde_json::to_value(&comments)?,
    }));
    Ok(())
}

/// `bcode claim <id>`（F07）：原子认领；被抢则业务错（退出码 6）。
/// 租约契约：认领后 2 小时无平台侧动作自动释放——长任务周期 `bcode log` 保活。
pub async fn claim(cfg: &Config, profile: &str, id: i64, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    ctx.client
        .post(&format!("/v1/tasks/{id}/claim"), json!({}))
        .await?;

    let mut payload = json!({ "claimed": true, "task_id": id });
    // 回显要点为增强体验；失败不影响认领结果
    match ctx
        .client
        .get_as::<TaskDetail>(&format!("/v1/tasks/{id}"))
        .await
    {
        Ok(d) => {
            out.kv("已认领", &d.one_line());
            payload["task"] = serde_json::to_value(&d)?;
        }
        Err(_) => out.kv("已认领", &format!("#{id}")),
    }
    out.kv(
        "租约",
        "2 小时无动作将自动释放，长任务请周期 bcode log 保活",
    );
    out.emit_value(&payload);
    Ok(())
}

/// `bcode complete <id>`（F08）：完成任务进 review；artifacts 为 markdown 产出。
pub async fn complete(
    cfg: &Config,
    profile: &str,
    id: i64,
    artifacts: Option<&str>,
    artifacts_file: Option<&str>,
    note: Option<&str>,
    out: &Out,
) -> Result<()> {
    let mut text = match (artifacts_file, artifacts) {
        (Some(path), _) => std::fs::read_to_string(path)
            .with_context(|| format!("读取 artifacts 文件 {path} 失败"))?,
        (None, Some(a)) => a.to_string(),
        (None, None) => String::new(),
    };
    if let Some(n) = note {
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&format!("## 备注\n{n}"));
    }

    let ctx = project_ctx(cfg, profile).await?;
    ctx.client
        .post(
            &format!("/v1/tasks/{id}/complete"),
            json!({ "artifacts": text }),
        )
        .await?;

    out.kv("已完成", &format!("#{id} → review（等待人工审核）"));
    out.emit_value(&json!({ "completed": true, "task_id": id }));
    Ok(())
}

/// `bcode log <id> <message>`（F09）：过程留痕（执行日志流）。
/// 同时是认领租约的心跳——平台侧动作会刷新 2h 租约。
pub async fn log(
    cfg: &Config,
    profile: &str,
    id: i64,
    message: &str,
    status: &str,
    action: &str,
    out: &Out,
) -> Result<()> {
    // 平台 schema 约束 CHECK(status IN ('success','failed'))——"running" 不可入库
    if !matches!(status, "success" | "failed") {
        bail!("--status 仅支持 success / failed（平台约束）");
    }
    let ctx = project_ctx(cfg, profile).await?;
    let created: CreatedId = ctx
        .client
        .post_as(
            &format!("/v1/tasks/{id}/ai-logs"),
            json!({
                "aiUserId": ctx.credential.agent_id,
                "action": action,
                "detail": message,
                "status": status,
            }),
        )
        .await?;

    out.kv("留痕", &format!("#{id} action={action} status={status}"));
    out.emit_value(&json!({ "logged": true, "log_id": created.id, "task_id": id }));
    Ok(())
}
