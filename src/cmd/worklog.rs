//! 工作日志域命令：项目演化的第一手记录（人类手写 + agent 草稿生成）。
//! 开工包只带最近 3 条摘录——完整历史走本命令族消费。
//! 写入需平台 worklog 能力位；draft 从已完成任务/发布生成草稿不落库。

use anyhow::{Context, Result, anyhow};
use serde_json::json;

use super::project_ctx;
use crate::cli::WorklogArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::worklog::{WorklogDraft, WorklogItem, WorklogList};
use crate::output::{Out, pad_display};

/// `bcode worklog`：缺省列表（倒序，id/日期/作者/来源/首行）。
/// `--show <id>` 单条完整内容（直连详情端点）；
/// `--write [--file <path>|--content <s>]` 写入（source 缺省 manual）；
/// `--draft --from --to` 生成草稿到 stdout（不落库；润色后 --write --source tasks 发布）。
pub async fn worklog(cfg: &Config, profile: &str, a: &WorklogArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    if let Some(id) = a.show {
        let item: WorklogItem = ctx.client.get_as(&format!("/v1/worklogs/{id}")).await?;
        render_item(&item, out, true);
        out.emit_value(&serde_json::to_value(&item)?);
        return Ok(());
    }

    if a.write {
        let content = match (a.file.as_deref(), a.content.as_deref()) {
            (Some(f), _) => std::fs::read_to_string(f).with_context(|| format!("读取 {f} 失败"))?,
            (None, Some(c)) => c.to_string(),
            (None, None) => return Err(bail_anyhow()),
        };
        let source = a.source.clone().unwrap_or_else(|| "manual".into());
        if !matches!(source.as_str(), "manual" | "tasks") {
            return Err(anyhow!(
                "--source 仅支持 manual / tasks（tasks=草稿润色后发布）"
            ));
        }
        let created: crate::model::task::CreatedId = ctx
            .client
            .post_as(
                &format!("/v1/projects/{pid}/worklogs"),
                json!({ "content": content, "source": source }),
            )
            .await?;
        out.kv("已记录", &format!("工作日志 #{}", created.id));
        out.emit_value(&json!({ "written": true, "worklog_id": created.id }));
        return Ok(());
    }

    if a.draft {
        let from = a
            .from
            .as_deref()
            .ok_or_else(|| anyhow!("--draft 需要 --from YYYY-MM-DD（配 --to，范围上限 92 天）"))?;
        let to =
            a.to.as_deref()
                .ok_or_else(|| anyhow!("--draft 需要 --to YYYY-MM-DD"))?;
        let d: WorklogDraft = ctx
            .client
            .get_as(&format!(
                "/v1/projects/{pid}/worklogs/draft?from={}&to={}",
                encode_query(from),
                encode_query(to)
            ))
            .await?;
        if !out.json {
            // 草稿直出 stdout：便于重定向润色（> draft.md 后编辑再 --write）
            println!("{}", d.content);
            eprintln!(
                "（草稿：{} 个任务 / {} 个发布；润色后 bcode worklog --write --file x.md --source tasks 发布）",
                d.task_count, d.release_count
            );
        }
        out.emit_value(&serde_json::to_value(&d)?);
        return Ok(());
    }

    // 缺省：列表（倒序分页）
    let page = a.page.unwrap_or(1);
    let size = a.size.unwrap_or(20);
    let list: WorklogList = ctx
        .sessioned_get(&format!(
            "/v1/projects/{pid}/worklogs?pageNum={page}&pageSize={size}"
        ))
        .await?;
    if list.list.is_empty() {
        out.line("（无工作日志）");
    }
    for item in &list.list {
        let author = if item.author_type == "ai" {
            "[AI] "
        } else {
            ""
        };
        let src = if item.source == "tasks" {
            "草稿"
        } else {
            "手写"
        };
        out.line(&format!(
            "  {} {author}{} [{}] {}（{}）",
            pad_display(&format!("#{}", item.id), 6),
            item.author_name,
            src,
            item.created_at,
            first_line(&item.content)
        ));
    }
    out.line(&format!("（共 {} 条）", list.total));
    out.emit_value(&serde_json::to_value(&list)?);
    Ok(())
}

fn render_item(item: &WorklogItem, out: &Out, full: bool) {
    let author = if item.author_type == "ai" {
        "[AI] "
    } else {
        ""
    };
    let src = if item.source == "tasks" {
        "草稿"
    } else {
        "手写"
    };
    out.line(&format!(
        "#{} [{}] {author}{}（{}）",
        item.id, src, item.author_name, item.created_at
    ));
    if full {
        out.line("");
        out.line(&item.content);
    }
}

fn first_line(s: &str) -> String {
    let l = s.lines().next().unwrap_or("");
    let mut cut: String = l.chars().take(60).collect();
    if l.chars().count() > 60 {
        cut.push('…');
    }
    cut
}

fn bail_anyhow() -> anyhow::Error {
    anyhow!("--write 需要 --file <路径> 或 --content <文本>")
}
