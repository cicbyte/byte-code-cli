//! 通信域命令（F11-F13）：comment 发评论 / comments 评论流 / notify 通知。

use anyhow::Result;
use futures_util::StreamExt;
use serde_json::{Value, json};

use super::{identity_client, project_ctx};
use crate::config::Config;
use crate::error::BcodeError;
use crate::model::platform::NotificationList;
use crate::model::task::{CommentList, CreatedId};
use crate::output::Out;

/// `bcode comment <taskId> <text>`（F11）：发表评论（支持 @提及）。
pub async fn comment(
    cfg: &Config,
    profile: &str,
    task_id: i64,
    text: &str,
    out: &Out,
) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let created: CreatedId = ctx
        .client
        .post_as(
            &format!("/v1/tasks/{task_id}/comments"),
            json!({ "content": text }),
        )
        .await?;
    out.kv("评论", &format!("#{task_id} → comment id={}", created.id));
    out.emit_value(&json!({ "comment_id": created.id, "task_id": task_id }));
    Ok(())
}

/// `bcode comments <taskId> [--follow]`（F12）：查看评论流；
/// --follow 增量轮询新评论（一行一事件，Ctrl-C 退出）。
pub async fn comments(
    cfg: &Config,
    profile: &str,
    task_id: i64,
    follow: bool,
    interval: u64,
    out: &Out,
) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let path = format!("/v1/tasks/{task_id}/comments");

    if !follow {
        let list: CommentList = ctx.client.get_as(&path).await?;
        if list.list.is_empty() {
            out.line("（无评论）");
        }
        for c in &list.list {
            out.line(&format!(
                "[{}] {}: {}",
                c.created_at,
                speaker(c.real_name.as_str(), c.username.as_str()),
                c.content
            ));
        }
        out.emit_value(&serde_json::to_value(&list)?);
        return Ok(());
    }

    // --follow：首拉全量，之后按 id 增量
    let mut last_id = 0i64;
    loop {
        let list: CommentList = ctx.client.get_as(&path).await?;
        for c in &list.list {
            if c.id > last_id {
                last_id = c.id;
                out.line(&format!(
                    "[{}] {}: {}",
                    c.created_at,
                    speaker(&c.real_name, &c.username),
                    c.content
                ));
                out.emit_event(&serde_json::to_value(c)?);
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(interval.max(1))).await;
    }
}

/// `bcode notify [--unread] [--watch] [--read <id>] [--read-all]`（F13 + v2）：
/// 通知列表 / 未读 / 标已读；--watch 走 SSE 实时流（一行一事件；
/// 指数退避重连 1s→60s，Ctrl-C 退出）。
pub async fn notify(
    cfg: &Config,
    profile: &str,
    a: &crate::cli::NotifyArgs,
    out: &Out,
) -> Result<()> {
    let client = identity_client(cfg, profile)?;

    // 已读操作优先分流（v2 反馈：unread 只读不可操作，永远堆积）
    if let Some(id) = a.read {
        client
            .put(&format!("/v1/notifications/{id}/read"), json!({}))
            .await?;
        out.kv("已读", &format!("#{id}"));
        out.emit_value(&json!({ "marked": id }));
        return Ok(());
    }
    if a.read_all {
        client.put("/v1/notifications/read-all", json!({})).await?;
        out.line("已全部标记已读");
        out.emit_value(&json!({ "marked_all": true }));
        return Ok(());
    }

    if !a.watch {
        let path = if a.unread {
            "/v1/notifications?unread=1&size=50"
        } else {
            "/v1/notifications?size=50"
        };
        let list: NotificationList = client.get_as(path).await?;
        if list.list.is_empty() {
            out.line("（无通知）");
        }
        for n in &list.list {
            let mark = if n.unread() { "●" } else { " " };
            out.line(&format!(
                "[{}] {mark} {} — {}",
                n.created_at, n.title, n.content
            ));
        }
        out.emit_value(&serde_json::to_value(&list)?);
        return Ok(());
    }

    // --watch：SSE 长连接，断线指数退避重连（决策记录 #3）
    let mut backoff_secs = 1u64;
    loop {
        match client.open_stream("/v1/notifications/stream").await {
            Ok(resp) => {
                backoff_secs = 1;
                let _ = consume_sse(resp, out).await; // 流结束/中断 → 走重连
            }
            Err(e) => eprintln!("bcode: {e:#}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
        backoff_secs = (backoff_secs * 2).min(60);
    }
}

/// 消费 SSE 流：事件以空行分隔，`data:` 行负载为通知 JSON；`: comment` 心跳行忽略。
/// 平台心跳 30s——90s 无任何字节视为静默断线，返回错误交给上层重连
async fn consume_sse(resp: reqwest::Response, out: &Out) -> Result<()> {
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    loop {
        let next = tokio::time::timeout(std::time::Duration::from_secs(90), stream.next()).await;
        let chunk = match next {
            // 空闲超时：连接半开/网络静默死亡，按断线处理
            Err(_) => {
                return Err(BcodeError::Network("SSE 流 90s 无数据（心跳丢失）".into()).into());
            }
            Ok(None) => return Ok(()),
            Ok(Some(Err(e))) => return Err(BcodeError::Network(format!("SSE 流中断：{e}")).into()),
            Ok(Some(Ok(c))) => c,
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buf.find("\n\n") {
            let block: String = buf.drain(..pos + 2).collect();
            if let Some(event) = parse_sse_event(&block) {
                // SSE 事件负载无 createdAt 字段（与列表接口不同构），缺省标「实时」
                let prefix = match event.get("createdAt").and_then(Value::as_str) {
                    Some(t) => format!("[{t}]"),
                    None => "[实时]".to_string(),
                };
                out.line(&format!(
                    "{prefix} {} — {}",
                    event.get("title").and_then(Value::as_str).unwrap_or(""),
                    event.get("content").and_then(Value::as_str).unwrap_or("")
                ));
                out.emit_event(&event);
            }
        }
    }
}

/// 解析单个 SSE 事件块：多条 data: 行按规范以 \n 拼接；无 data 行（心跳注释）返回 None
fn parse_sse_event(block: &str) -> Option<Value> {
    let data: Vec<&str> = block
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(|l| l.trim_start())
        .collect();
    if data.is_empty() {
        return None;
    }
    serde_json::from_str(&data.join("\n")).ok()
}

fn speaker(real_name: &str, username: &str) -> String {
    if real_name.is_empty() {
        username.to_string()
    } else {
        real_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_data_event_parses() {
        let v = parse_sse_event("data: {\"id\":1,\"title\":\"t\"}\n\n").unwrap();
        assert_eq!(v["id"], 1);
    }

    #[test]
    fn sse_heartbeat_comment_yields_none() {
        assert!(parse_sse_event(": ping\n\n").is_none());
        assert!(parse_sse_event("\n").is_none());
    }
}
