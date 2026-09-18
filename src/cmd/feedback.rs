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
    // 已发送视图：身份级端点（created_by=当前 agent），不依赖目录指向
    if a.sent {
        let client = super::identity_client(cfg, profile)?;
        #[derive(serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SentItem {
            #[serde(default)]
            id: i64,
            #[serde(default)]
            title: String,
            #[serde(default)]
            status: String,
            #[serde(default)]
            target_project_id: i64,
            #[serde(default)]
            target_project_name: String,
            #[serde(default)]
            source_task_id: i64,
            #[serde(default)]
            converted_task_id: i64,
            #[serde(default)]
            dismiss_reason: String,
            #[serde(default)]
            created_at: String,
        }
        #[derive(serde::Serialize, serde::Deserialize)]
        struct SentList {
            #[serde(default, deserialize_with = "crate::model::null_to_default")]
            list: Vec<SentItem>,
        }
        let status = a.status.as_deref().unwrap_or("open");
        let res: SentList = client
            .get_as(&format!(
                "/v1/feedbacks/sent?status={}",
                encode_query(status)
            ))
            .await?;
        if res.list.is_empty() {
            out.line("（无已发反馈）");
        }
        for f in &res.list {
            let tail = match f.status.as_str() {
                "converted" => format!("（已转任务 #{}）", f.converted_task_id),
                "dismissed" => format!("（被忽略：{}）", f.dismiss_reason),
                _ => String::new(),
            };
            let lineage = if f.source_task_id > 0 {
                format!(" ←任务#{}", f.source_task_id)
            } else {
                String::new()
            };
            out.line(&format!(
                "  {} [{}] {}（→ {}{}）{}",
                pad_display(&format!("#{}", f.id), 6),
                f.status,
                f.title,
                f.target_project_name,
                lineage,
                tail
            ));
        }
        out.emit_value(&serde_json::to_value(&res)?);
        return Ok(());
    }

    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    // 投递：目标解析（名称→id 便捷层；解析不到时报错并指引——平台门禁对
    // 关联/分组的判定以数字 id 到达后为准，CLI 不做比平台更严的预检）
    if let Some(to) = &a.send {
        let target = resolve_target(&ctx.client, pid, to).await?.ok_or_else(|| {
            anyhow!(
                "无法解析目标「{to}」：不是数字 id，也不在关联列表/已加入项目中。\n\
                     可用 bcode projects 查看已加入项目（名称/短码均可作 --send 值）"
            )
        })?;
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
        if let Some(sp) = a.source {
            body["sourceProjectId"] = json!(sp);
        }
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(&format!("/v1/projects/{target}/feedbacks"), body)
            .await?;
        out.kv(
            "已投递",
            &format!(
                "反馈 #{} → 项目 id={target}（状态可用 feedback --sent 追踪）",
                created.id
            ),
        );
        out.emit_value(&json!({ "sent": created.id, "to_project": target }));
        return Ok(());
    }

    if let Some(id) = &a.convert {
        let mut body = json!({});
        if let Some(t) = a.title.as_deref() {
            body["title"] = json!(t);
        }
        // 响应字段是 taskId（FeedbackConvertRes），非通用 {id}——曾用 CreatedId
        // 解析致 serde default 0 显示「任务 #0」（平台反馈 #2 报告，任务 #15 修复）
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ConvertResult {
            #[serde(default)]
            task_id: i64,
        }
        let created: ConvertResult = ctx
            .client
            .post_as(&format!("/v1/projects/{pid}/feedbacks/{id}/convert"), body)
            .await?;
        out.kv(
            "已转任务",
            &format!("反馈 #{id} → 任务 #{}", created.task_id),
        );
        out.emit_value(&json!({ "converted_feedback": id, "task_id": created.task_id }));
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

/// 名称→目标项目 id 解析（预检降级版，反馈 #12）：数字直通；名称在
/// 显式关联列表与**agent 已加入项目**里查——查不到返回 None 而非报错。平台
/// 对反馈的权威判定是「显式关联 **或** ShareGroup 共组」，CLI 预检不该比
/// 平台更严（曾因此误拦合法投递）；解析失败让平台门禁裁决。
/// **显式关联列表**里查——查不到返回 None 而非报错。平台对反馈的权威判定
/// 是「显式关联 **或** ShareGroup 共组」，分组项目不在 relations 里，CLI
/// 预检不该比平台更严（曾因此误拦合法投递）；解析失败让平台门禁裁决。
async fn resolve_target(
    client: &crate::client::BcodeClient,
    pid: i64,
    spec: &str,
) -> Result<Option<i64>> {
    if let Ok(id) = spec.parse::<i64>() {
        return Ok(Some(id));
    }
    let rels: RelationList = client
        .get_as(&format!("/v1/projects/{pid}/relations"))
        .await?;
    if let Some(r) = rels.list.iter().find(|r| r.name == spec) {
        return Ok(Some(r.project_id));
    }
    // 兜底：agent 已加入项目清单（GET /agent/projects）——覆盖「已 join 但
    // 未配显式关联」的目标（如分组伙伴），拿不到 id 时才放弃解析
    #[derive(serde::Deserialize)]
    struct AgentProjects {
        #[serde(default, deserialize_with = "crate::model::null_to_default")]
        list: Vec<crate::model::agent::ProjectBrief>,
    }
    let joined: AgentProjects = client.get_as("/v1/agent/projects").await?;
    Ok(joined
        .list
        .iter()
        .find(|p| p.name == spec || p.code == spec)
        .map(|p| p.id))
}
