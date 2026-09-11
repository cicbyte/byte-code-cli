//! 专题域命令：长期任务的阶段化推进（PRD 拆解 → 阶段 → 转日常任务池）。
//! 平台注释明确点名 `bcode topic work` 为 agent 推进入口。

use anyhow::{Result, anyhow, bail};
use serde_json::json;

use super::project_ctx;
use crate::cli::TopicArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::topic::{TopicDetailRes, TopicList};
use crate::output::{Out, pad_display};

/// `bcode topic`（专题域全生命周期）：缺省列表 / `--create` 创建（默认自任执行）/
/// `--phases <tid> --file f` PRD 拆解批量导入阶段（整体替换）/
/// `--detail <id>` 详情 / `--work --phase --next` 推进 /
/// `--log --action handoff` 留痕 / `--convert` 阶段转任务 / `--finish` 终验收（人）
pub async fn topic(cfg: &Config, profile: &str, a: &TopicArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    // 创建专题（v5：平台 assigneeId 语义即执行 agent，默认自任）
    if a.create {
        let title = a
            .title
            .as_deref()
            .ok_or_else(|| anyhow!("创建专题需要 --title"))?;
        let mut body = json!({ "title": title });
        if let Some(g) = a.goal.as_deref() {
            body["goal"] = json!(g);
        }
        if let Some(ac) = a.acceptance.as_deref() {
            body["acceptance"] = json!(ac);
        }
        if let Some(d) = a.doc_path.as_deref() {
            body["docPath"] = json!(d);
        }
        body["assigneeId"] = json!(a.assignee.unwrap_or(ctx.credential.agent_id));
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(&format!("/v1/projects/{pid}/topics"), body)
            .await?;
        out.kv(
            "已创建专题",
            &format!(
                "#{} {title}（执行 agent 已按 --assignee/自任 设置）",
                created.id
            ),
        );
        out.emit_value(&json!({ "created": true, "topic_id": created.id, "title": title }));
        return Ok(());
    }

    // 阶段批量导入（PRD 拆解 → 整体替换写入；平台注释明确此为 agent 职责）
    if let Some(tid) = a.phases {
        let f = a
            .file
            .as_deref()
            .ok_or_else(|| anyhow!("--phases 需要 --file <阶段清单 JSON 文件>"))?;
        let raw = std::fs::read_to_string(f).map_err(|e| anyhow!("读取 {f} 失败：{e}"))?;
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&raw).map_err(|e| anyhow!("{f} 不是合法 JSON 数组：{e}"))?;
        ctx.client
            .post(
                &format!("/v1/projects/{pid}/topics/{tid}/phases"),
                json!({ "phases": parsed }),
            )
            .await?;
        out.kv(
            "已导入阶段",
            &format!("专题 #{tid} ← {} 个阶段（整体替换）", parsed.len()),
        );
        out.emit_value(&json!({ "topic": tid, "phases": parsed.len() }));
        return Ok(());
    }

    // 阶段推进（平台注释点名的 agent 入口）
    if let (Some(tid), Some(phid)) = (a.work, a.phase) {
        let status = match (a.next, a.status.as_deref()) {
            (true, Some(_)) => bail!("--next 与 --status 互斥"),
            (true, None) => {
                // 自动推进：pending→in_progress→done
                let detail: TopicDetailRes = ctx
                    .client
                    .get_as(&format!("/v1/projects/{pid}/topics/{tid}"))
                    .await?;
                let ph = detail
                    .topic
                    .phases
                    .iter()
                    .find(|p| p.id == phid)
                    .ok_or_else(|| anyhow!("专题 #{tid} 无阶段 #{phid}"))?;
                match ph.status.as_str() {
                    "pending" => "in_progress",
                    "in_progress" => "done",
                    "done" => bail!("阶段 #{phid} 已完成，无下一状态"),
                    other => bail!("未知阶段状态：{other}"),
                }
            }
            (false, Some(s)) => s,
            (false, None) => {
                bail!("work 需要 --next（自动推进）或 --status <pending|in_progress|done>")
            }
        };
        ctx.client
            .post(
                &format!("/v1/projects/{pid}/topics/{tid}/phases/{phid}/toggle"),
                json!({ "status": status }),
            )
            .await?;
        out.kv("阶段推进", &format!("#{tid}/阶段#{phid} → {status}"));
        out.emit_value(&json!({ "topic": tid, "phase": phid, "status": status }));
        return Ok(());
    }

    // 阶段转日常任务（血缘反向关联）。响应字段是 taskId（同 feedback convert 的坑）
    if let (Some(tid), Some(phid)) = (a.convert, a.phase) {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ConvertResult {
            #[serde(default)]
            task_id: i64,
        }
        let created: ConvertResult = ctx
            .client
            .post_as(
                &format!("/v1/projects/{pid}/topics/{tid}/phases/{phid}/convert"),
                json!({}),
            )
            .await?;
        out.kv(
            "阶段转任务",
            &format!("#{tid}/阶段#{phid} → 任务 #{}", created.task_id),
        );
        out.emit_value(&json!({ "topic": tid, "phase": phid, "task_id": created.task_id }));
        return Ok(());
    }

    // 留痕：progress=执行进展；handoff=交接摘要（下个会话恢复点）
    if let Some(id) = a.log {
        let detail = a
            .detail_text
            .as_deref()
            .ok_or_else(|| anyhow!("log 需要正文：--detail-text <内容>"))?;
        let action = a.action.as_deref().unwrap_or("progress");
        if !matches!(action, "progress" | "handoff") {
            bail!("--action 仅支持 progress / handoff");
        }
        ctx.client
            .post(
                &format!("/v1/projects/{pid}/topics/{id}/log"),
                json!({ "action": action, "detail": detail }),
            )
            .await?;
        out.kv("专题留痕", &format!("#{id} action={action}"));
        out.emit_value(&json!({ "topic": id, "logged": true, "action": action }));
        return Ok(());
    }

    // 终验收（平台语义为「人」执行；agent 调用被拒属预期）
    if let Some(id) = a.finish {
        let result = a
            .result
            .as_deref()
            .ok_or_else(|| anyhow!("finish 需要 --result completed|abandoned"))?;
        if !matches!(result, "completed" | "abandoned") {
            bail!("--result 仅支持 completed / abandoned");
        }
        ctx.client
            .post(
                &format!("/v1/projects/{pid}/topics/{id}/finish"),
                json!({ "result": result }),
            )
            .await?;
        out.kv("终验收", &format!("#{id} → {result}"));
        out.emit_value(&json!({ "topic": id, "finished": result }));
        return Ok(());
    }

    // 单专题详情（含阶段清单与最近 handoff）
    if let Some(id) = a.detail {
        let d: TopicDetailRes = ctx
            .client
            .get_as(&format!("/v1/projects/{pid}/topics/{id}"))
            .await?;
        let t = &d.topic;
        out.line(&format!("#{} {} [{}]", t.id, t.title, t.status));
        out.kv("目标", &t.goal);
        if !t.acceptance.is_empty() {
            out.kv("验收", &t.acceptance);
        }
        if !t.doc_path.is_empty() {
            out.kv("文档", &t.doc_path);
        }
        out.kv("进度", &format!("{}/{} 阶段", t.phase_done, t.phase_total));
        if !t.last_handoff.is_empty() {
            out.kv("最近交接", &t.last_handoff);
        }
        out.line("");
        out.line("── 阶段 ──");
        for p in &t.phases {
            let task = if p.task_id > 0 {
                format!(" → 任务#{}", p.task_id)
            } else {
                String::new()
            };
            out.line(&format!(
                "  {} [{}] {}{task}",
                pad_display(&format!("#{}", p.id), 6),
                p.status,
                p.title
            ));
        }
        out.emit_value(&serde_json::to_value(&d)?);
        return Ok(());
    }

    // 缺省：专题列表（默认 active）
    let status = if a.all { "all" } else { "active" };
    let list: TopicList = ctx
        .sessioned_get(&format!(
            "/v1/projects/{pid}/topics?status={}",
            encode_query(status)
        ))
        .await?;
    if list.list.is_empty() {
        out.line("（无进行中专题——bcode topic <id> 看详情，work 推进阶段）");
    }
    for t in &list.list {
        out.line(&format!(
            "  {} {}/{} {} [{}]",
            pad_display(&format!("#{}", t.id), 6),
            t.phase_done,
            t.phase_total,
            t.title,
            t.status
        ));
    }
    out.emit_value(&serde_json::to_value(&list)?);
    Ok(())
}
