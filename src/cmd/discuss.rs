//! 讨论区域命令：项目内论坛式想法/议题线程——agent 参与讨论的入口
//! （发起/回复需平台 discuss 能力位；列表/详情读全成员）。
//! 想法 → 讨论 → convert 转任务（血缘互链），是任务先行的轻量前置形态。

use anyhow::{Context, Result, anyhow};
use serde_json::json;

use super::project_ctx;
use crate::cli::DiscussArgs;
use crate::config::Config;
use crate::model::discuss::{DiscussionDetail, DiscussionList};
use crate::output::{Out, pad_display};

/// `bcode discuss [id]`：缺省列表；带 id 详情（含回复）。
/// `--new --title <t> [--file body.md|--body <s>]` 发起；
/// `--reply <文本>` / `--reply-file` 回复指定讨论（id 位置参数）；
/// `--convert [--title] [--type]` 转任务；`--archive` 归档/恢复切换。
pub async fn discuss(cfg: &Config, profile: &str, a: &DiscussArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    // 发起讨论
    if a.new {
        let title = a
            .title
            .as_deref()
            .ok_or_else(|| anyhow!("发起讨论需要 --title"))?;
        let body = match (a.file.as_deref(), a.body.as_deref()) {
            (Some(f), _) => std::fs::read_to_string(f).with_context(|| format!("读取 {f} 失败"))?,
            (None, Some(b)) => b.to_string(),
            (None, None) => String::new(),
        };
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(
                &format!("/v1/projects/{pid}/discussions"),
                json!({ "title": title, "body": body }),
            )
            .await?;
        out.kv("已发起", &format!("讨论 #{} {title}", created.id));
        out.emit_value(&json!({ "created": true, "discussion_id": created.id }));
        return Ok(());
    }

    let id = match a.id {
        Some(i) => i,
        None => {
            // 缺省：活跃讨论列表
            let list: DiscussionList = ctx
                .sessioned_get(&format!(
                    "/v1/projects/{pid}/discussions?pageNum=1&pageSize=50"
                ))
                .await?;
            if list.list.is_empty() {
                out.line("（无活跃讨论——bcode discuss --new --title 发起）");
            }
            for d in &list.list {
                let author = if d.author_type == "ai" { "[AI]" } else { "" };
                let tag = if d.status == "converted" {
                    "→ 已转任务"
                } else {
                    ""
                };
                out.line(&format!(
                    "  {} {author}{} [{}] {}（{} 回复）{tag}",
                    pad_display(&format!("#{}", d.id), 6),
                    d.author_name,
                    d.status,
                    d.title,
                    d.reply_count
                ));
            }
            out.emit_value(&serde_json::to_value(&list)?);
            return Ok(());
        }
    };

    // 回复
    if a.reply.is_some() || a.reply_file.is_some() {
        let content = match (a.reply_file.as_deref(), a.reply.as_deref()) {
            (Some(f), _) => std::fs::read_to_string(f).with_context(|| format!("读取 {f} 失败"))?,
            (None, Some(r)) => r.to_string(),
            (None, None) => unreachable!(),
        };
        ctx.client
            .post(
                &format!("/v1/discussions/{id}/replies"),
                json!({ "content": content }),
            )
            .await?;
        out.kv("已回复", &format!("讨论 #{id}"));
        out.emit_value(&json!({ "replied": true, "discussion_id": id }));
        return Ok(());
    }

    // 转任务（血缘互链：讨论标 converted + converted_task_id）
    if a.convert {
        let mut body = json!({});
        if let Some(t) = a.title.as_deref() {
            body["title"] = json!(t);
        }
        if let Some(t) = a.r#type.as_deref() {
            body["type"] = json!(t);
        }
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ConvertResult {
            #[serde(default)]
            task_id: i64,
        }
        let created: ConvertResult = ctx
            .client
            .post_as(&format!("/v1/discussions/{id}/convert"), body)
            .await?;
        out.kv(
            "已转任务",
            &format!("讨论 #{id} → 任务 #{}", created.task_id),
        );
        out.emit_value(
            &json!({ "converted": true, "discussion_id": id, "task_id": created.task_id }),
        );
        return Ok(());
    }

    // 归档/恢复切换
    if a.archive {
        ctx.client
            .post(&format!("/v1/discussions/{id}/archive"), json!({}))
            .await?;
        out.kv("已切换", &format!("讨论 #{id} 归档状态（archived↔open）"));
        out.emit_value(&json!({ "archived_toggled": true, "discussion_id": id }));
        return Ok(());
    }

    // 详情（含回复时间线）
    let d: DiscussionDetail = ctx.client.get_as(&format!("/v1/discussions/{id}")).await?;
    let author = if d.d.author_type == "ai" { "[AI] " } else { "" };
    out.line(&format!(
        "#{} {} [{}] {}（{} 回复）",
        d.d.id, author, d.d.status, d.d.title, d.d.reply_count
    ));
    out.kv("作者", &format!("{author}{}", d.d.author_name));
    out.kv("更新", &d.d.updated_at);
    if !d.d.body.is_empty() {
        out.line("");
        out.line("── 正文 ──");
        out.line(&d.d.body);
    }
    out.line("");
    out.line("── 回复 ──");
    if d.replies.is_empty() {
        out.line("  （无）");
    }
    for r in &d.replies {
        let ut = if r.user_type == "ai" { "[AI] " } else { "" };
        out.line(&format!(
            "  [{}] {ut}{}：{}",
            r.created_at, r.user_name, r.content
        ));
    }
    out.emit_value(&serde_json::to_value(&d)?);
    Ok(())
}
