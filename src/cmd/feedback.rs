//! 跨项目反馈域命令：agent 发现其他项目的问题时投递反馈，
//! 接收方阅读后 convert 建任务（血缘回填）或 dismiss 忽略（理由回告）。

use anyhow::{Result, anyhow};
use serde_json::json;

use super::project_ctx;
use crate::cli::FeedbackArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::feedback::{FeedbackList, RelationList};
use crate::output::{Out, pad_display};

/// `bcode feedback send --to <关联项目 code|id|名> --title [--content|--file] [--task <id>]`
/// `feedback list [--status open|all]` / `feedback convert <id> [--title]` / `feedback dismiss <id> --reason`
pub async fn feedback(cfg: &Config, profile: &str, a: &FeedbackArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    // 投递：目标必须是本项目的关联项目（owner 在 Web 配置关联）
    if let Some(to) = &a.send {
        let target = resolve_relation(&ctx.client, pid, to).await?;
        let content = match (a.file.as_deref(), a.content.as_deref()) {
            (Some(f), _) => {
                std::fs::read_to_string(f).map_err(|e| anyhow!("读取 {f} 失败：{e}"))?
            }
            (None, Some(c)) => c.to_string(),
            (None, None) => String::new(),
        };
        let title = a
            .title
            .as_deref()
            .ok_or_else(|| anyhow!("send 需要 --title"))?;
        let mut body = json!({ "title": title, "content": content });
        if let Some(t) = a.task {
            body["sourceTaskId"] = json!(t);
        }
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(&format!("/v1/projects/{}/feedbacks", target), body)
            .await?;
        out.kv(
            "已投递",
            &format!("反馈 #{} → 关联项目 id={}", created.id, target),
        );
        out.emit_value(&json!({ "sent": created.id, "to_project": target }));
        return Ok(());
    }

    if let Some(id) = &a.convert {
        let mut body = json!({});
        if let Some(t) = a.title.as_deref() {
            body["title"] = json!(t);
        }
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(&format!("/v1/projects/{pid}/feedbacks/{id}/convert"), body)
            .await?;
        out.kv("已转任务", &format!("反馈 #{id} → 任务 #{}", created.id));
        out.emit_value(&json!({ "converted_feedback": id, "task_id": created.id }));
        return Ok(());
    }

    if let Some(id) = &a.dismiss {
        let reason = a
            .reason
            .as_deref()
            .ok_or_else(|| anyhow!("dismiss 必须给 --reason（会回告发起方）"))?;
        ctx.client
            .post(
                &format!("/v1/projects/{pid}/feedbacks/{id}/dismiss"),
                json!({ "reason": reason }),
            )
            .await?;
        out.kv("已忽略", &format!("反馈 #{id}（理由已回告发起方）"));
        out.emit_value(&json!({ "dismissed": id }));
        return Ok(());
    }

    // 缺省：收件箱
    let status = a.status.as_deref().unwrap_or("open");
    let list: FeedbackList = ctx
        .sessioned_get(&format!(
            "/v1/projects/{pid}/feedbacks?status={}&size=50",
            encode_query(status)
        ))
        .await?;
    if list.list.is_empty() {
        out.line("（无待处理反馈）");
    }
    for f in &list.list {
        out.line(&format!(
            "  {} [{}] {}（来自 {}）",
            pad_display(&format!("#{}", f.id), 6),
            f.status,
            f.title,
            f.source_project_name
        ));
    }
    out.emit_value(&serde_json::to_value(&list)?);
    Ok(())
}

/// 从关联项目列表解析目标（id / 名称模糊匹配；数字直通）
async fn resolve_relation(
    client: &crate::client::BcodeClient,
    pid: i64,
    spec: &str,
) -> Result<i64> {
    if let Ok(id) = spec.parse::<i64>() {
        return Ok(id);
    }
    let rels: RelationList = client
        .get_as(&format!("/v1/projects/{pid}/relations"))
        .await?;
    rels.list
        .iter()
        .find(|r| r.name == spec)
        .map(|r| r.project_id)
        .ok_or_else(|| {
            anyhow!(
                "「{spec}」不在本项目关联列表——反馈只能投递给关联项目（owner 在 Web 配置项目关联）"
            )
        })
}
