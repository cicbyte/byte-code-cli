//! 任务域命令（工作会话层）：tasks 列表 / task 详情 / create 建任务 /
//! update 改字段 / claim 认领 / complete 完成 / log 过程留痕。

use anyhow::{Context, Result, bail};
use serde_json::json;

use super::project_ctx;
use crate::cli::UpdateArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::agent::{AgentTasks, TaskBrief};
use crate::model::task::{AiLogList, CommentList, CreatedId, TaskDetail};
use crate::output::{Out, pad_display};

/// `bcode tasks [--status s] [--keyword kw] [--priority p] [--sort s]`（F06）：
/// 免参任务列表。status 透传平台语义：缺省=未完成三态，all=全部。
/// 默认按优先级升序（P1 在前）+ id 降序，`--sort id` 恢复平台原始顺序。
pub async fn tasks(
    cfg: &Config,
    profile: &str,
    status: Option<&str>,
    keyword: Option<&str>,
    priority: Option<i64>,
    sort: &str,
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
    // 免参端点走 sessioned_get：缓存会话失效时自愈重建（见 Ctx::sessioned_get）
    let mut page: AgentTasks = ctx.sessioned_get(&path).await?;

    if let Some(p) = priority {
        page.list.retain(|t| t.priority == p);
        page.total = page.list.len() as i64;
    }
    sort_tasks(&mut page.list, sort);

    if page.list.is_empty() {
        out.line(&format!("（无任务，共 {} 条记录）", page.total));
    } else {
        // 列：id/状态/类型/优先/截止/标签/标题（type/tags 为 v3 平台新增字段）
        let head = format!(
            "  {} {} {} {} {} {} {}",
            pad_display("id", 7),
            pad_display("状态", 13),
            pad_display("类型", 8),
            pad_display("优先", 4),
            pad_display("截止", 12),
            pad_display("标签", 10),
            "标题"
        );
        out.line(&head);
        for t in &page.list {
            let due = t.due_date.as_deref().unwrap_or("—");
            let tags = if t.tags.is_empty() {
                "—".to_string()
            } else {
                t.tags.join(",")
            };
            out.line(&format!(
                "  {} {} {} {} {} {} {}",
                pad_display(&t.id.to_string(), 7),
                pad_display(&t.status, 13),
                pad_display(
                    if t.r#type.is_empty() {
                        "—"
                    } else {
                        &t.r#type
                    },
                    8
                ),
                pad_display(&format!("P{}", t.priority), 4),
                pad_display(due, 12),
                pad_display(&tags, 10),
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
    if !detail.created_at.is_empty() {
        out.kv("创建时间", &detail.created_at);
    }
    if !detail.updated_at.is_empty() {
        out.kv("更新时间", &detail.updated_at);
    }
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

/// `bcode release <id>`（v3 新增，平台 316fbd5）：认领人主动放回任务池——
/// 认领错了/依赖阻塞时即刻释出，不必干等 2h 租约；仅 assignee 本人，留痕 released。
pub async fn release(cfg: &Config, profile: &str, id: i64, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    ctx.client
        .post(&format!("/v1/tasks/{id}/release"), json!({}))
        .await?;
    out.kv("已释放", &format!("#{id} → open（任务回池，他人可认领）"));
    out.emit_value(&json!({ "released": true, "task_id": id }));
    Ok(())
}

/// `bcode reopen <id> --reason <原因>`（v3 新增）：终态任务（done/closed）重开 → open，
/// 原因必填（留痕）。复核不通过/回归问题时的回退路径。
pub async fn reopen(cfg: &Config, profile: &str, id: i64, reason: &str, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    ctx.client
        .post(
            &format!("/v1/tasks/{id}/reopen"),
            json!({ "reason": reason }),
        )
        .await?;
    out.kv("已重开", &format!("#{id} → open（原因已留痕）"));
    out.emit_value(&json!({ "reopened": true, "task_id": id, "reason": reason }));
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
    // 平台侧无 artifacts 长度校验（DB TEXT），CLI 兜底防误传超大文件整段上传
    const MAX_ARTIFACTS_BYTES: usize = 1024 * 1024;
    if text.len() > MAX_ARTIFACTS_BYTES {
        bail!(
            "artifacts 过大（{} 字节，上限 1 MiB）：请精简正文或拆分后用 log 补充",
            text.len()
        );
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

/// `bcode create`（实验性，v1 反馈新增）：建任务——CLI 侧补齐建任务入口，
/// 绕开 Windows 下 curl 内联中文的编码坑（--file 由 Rust 按 UTF-8 读取、
/// argv 天然无代码页问题）。请求体字段即平台 TaskCreateReq。
pub async fn create(
    cfg: &Config,
    profile: &str,
    a: &crate::cli::CreateArgs,
    out: &Out,
) -> Result<()> {
    // 请求体组装在联网前：缺 title 立即报错，不浪费一次握手
    let mut body = match a.file.as_deref() {
        Some(p) => serde_json::from_str(
            &std::fs::read_to_string(p).with_context(|| format!("读取 {p} 失败"))?,
        )
        .with_context(|| format!("解析 {p} 为 JSON 失败"))?,
        None => json!({}),
    };
    if let Some(t) = a.title.as_deref() {
        body["title"] = json!(t);
    }
    if let Some(d) = a.description.as_deref() {
        body["description"] = json!(d);
    }
    if let Some(t) = a.r#type.as_deref() {
        body["type"] = json!(t);
    }
    if let Some(p) = a.priority {
        body["priority"] = json!(p);
    }
    if let Some(d) = a.due.as_deref() {
        body["dueDate"] = json!(d);
    }
    let title = body
        .get("title")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("title 必填：--title <标题> 或 --file JSON 内提供"))?;

    let ctx = project_ctx(cfg, profile).await?;
    let created: CreatedId = ctx
        .client
        .post_as(
            &format!("/v1/projects/{}/tasks", ctx.project.project_id),
            body.clone(),
        )
        .await?;

    out.kv("已创建", &format!("#{} {title}", created.id));
    out.emit_value(&json!({ "created": true, "task_id": created.id, "title": title }));
    Ok(())
}

/// `bcode update <id>`（v2 反馈新增）：改任务字段。平台门禁：agent 禁改
/// status/assigneeId（状态流转必须走 claim/complete），CLI 不暴露这两项。
/// 指针语义：只提交出现的字段，其余不动；--due "" 传空串=清除截止。
pub async fn update(cfg: &Config, profile: &str, a: &UpdateArgs, out: &Out) -> Result<()> {
    let mut body = json!({});
    if let Some(t) = a.title.as_deref() {
        body["title"] = json!(t);
    }
    if let Some(d) = a.description.as_deref() {
        body["description"] = json!(d);
    }
    if let Some(t) = a.r#type.as_deref() {
        body["type"] = json!(t);
    }
    if let Some(p) = a.priority {
        body["priority"] = json!(p);
    }
    if let Some(d) = a.due.as_deref() {
        body["dueDate"] = json!(d);
    }
    if body.as_object().is_none_or(|m| m.is_empty()) {
        bail!(
            "未指定修改项：--title/--description/--type/--priority/--due 至少一个（状态流转走 claim/complete，改派是 owner 权限）"
        );
    }

    let ctx = project_ctx(cfg, profile).await?;
    ctx.client.put(&format!("/v1/tasks/{}", a.id), body).await?;
    // 回显要点（失败不影响更新结果）
    let mut payload = json!({ "updated": true, "task_id": a.id });
    match ctx
        .client
        .get_as::<TaskDetail>(&format!("/v1/tasks/{}", a.id))
        .await
    {
        Ok(d) => {
            out.kv("已更新", &d.one_line());
            payload["task"] = serde_json::to_value(&d)?;
        }
        Err(_) => out.kv("已更新", &format!("#{}", a.id)),
    }
    out.emit_value(&payload);
    Ok(())
}

/// 列表排序：priority=优先级升序（P1 在前）+ id 降序；id=平台原始顺序（id 降序）
fn sort_tasks(list: &mut [TaskBrief], sort: &str) {
    match sort {
        "id" => list.sort_by_key(|t| std::cmp::Reverse(t.id)),
        _ => list.sort_by_key(|t| (t.priority, std::cmp::Reverse(t.id))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief(id: i64, priority: i64) -> TaskBrief {
        TaskBrief {
            id,
            title: format!("t{id}"),
            status: "open".into(),
            r#type: "chore".into(),
            priority,
            tags: vec![],
            due_date: None,
            updated_at: String::new(),
        }
    }

    #[test]
    fn sort_puts_low_priority_number_first() {
        let mut list = vec![brief(30, 3), brief(31, 1), brief(25, 1), brief(28, 2)];
        sort_tasks(&mut list, "priority");
        let ids: Vec<i64> = list.iter().map(|t| t.id).collect();
        // P1 组内 id 降序，然后 P2、P3
        assert_eq!(ids, vec![31, 25, 28, 30]);

        let mut raw = vec![brief(1, 5), brief(9, 1)];
        sort_tasks(&mut raw, "id");
        assert_eq!(raw[0].id, 9);
    }
}
