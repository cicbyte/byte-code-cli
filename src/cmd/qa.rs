//! QA 库域命令：常见问答沉淀与检索（agent 维护 + 检索）。
//! 遇到问题先查 QA 库再问人；踩坑后 qa add 沉淀，qa hit 计数影响开工包 Top 排序。

use anyhow::{Result, anyhow};
use serde_json::json;

use super::project_ctx;
use crate::cli::QaArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::qa::{QaList, QaUpsertResult};
use crate::output::{Out, pad_display};

/// `bcode qa [keyword] [--tag t]`：QA 列表/搜索（按命中数降序）；
/// `qa add --question --answer`：沉淀（同问题 upsert）；
/// `qa hit <id>`：命中计数（查阅某条后调用）；
/// `qa archive <id>`：归档过期条目。
pub async fn qa(cfg: &Config, profile: &str, a: &QaArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    if let Some(q) = &a.question {
        let answer = a
            .answer
            .as_deref()
            .ok_or_else(|| anyhow!("沉淀 QA 需要 --answer 提供答案"))?;
        let mut body = json!({ "question": q, "answer": answer });
        if let Some(t) = a.tags.as_deref() {
            body["tags"] = json!(t);
        }
        let res: QaUpsertResult = ctx
            .client
            .post_as(&format!("/v1/projects/{pid}/qas"), body)
            .await?;
        let mode = if res.updated {
            "更新已有条目"
        } else {
            "新建"
        };
        out.kv("QA", &format!("#{} {mode}", res.id));
        out.emit_value(&serde_json::to_value(&res)?);
        return Ok(());
    }

    if let Some(id) = a.hit {
        ctx.client
            .post(&format!("/v1/projects/{pid}/qas/{id}/hit"), json!({}))
            .await?;
        out.kv("命中计数", &format!("#{id} +1（影响开工包 Top 排序）"));
        out.emit_value(&json!({ "hit": id }));
        return Ok(());
    }

    if let Some(id) = a.archive {
        ctx.client
            .post(&format!("/v1/projects/{pid}/qas/{id}/archive"), json!({}))
            .await?;
        out.kv("已归档", &format!("#{id}"));
        out.emit_value(&json!({ "archived": id }));
        return Ok(());
    }

    // 缺省：列表/搜索
    let mut path = format!("/v1/projects/{pid}/qas?size=50");
    if let Some(kw) = &a.keyword {
        path.push_str(&format!("&keyword={}", encode_query(kw)));
    }
    if let Some(t) = &a.tag {
        path.push_str(&format!("&tag={}", encode_query(t)));
    }
    let list: QaList = ctx.sessioned_get(&path).await?;
    if list.list.is_empty() {
        out.line("（QA 库为空或无命中——踩坑后 bcode qa add 沉淀，避免重复求解）");
    }
    for q in &list.list {
        out.line(&format!(
            "  {} {} [{}次]",
            pad_display(&format!("#{}", q.id), 6),
            q.question,
            q.hits
        ));
    }
    out.emit_value(&serde_json::to_value(&list)?);
    Ok(())
}
