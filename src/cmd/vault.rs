//! 上下文消费域命令（F15-F17 + v2 写侧扩展）：docs 文档中枢（读+写）/
//! memory·memories 项目记忆（读+写）/ search 全局搜索。

use anyhow::{Context, Result, anyhow, bail};
use serde_json::json;

use super::{identity_client, project_ctx};
use crate::cli::MemoryArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::docs::{
    DocFile, DocWriteResult, MemoryItem, MemoryList, VaultNode, VaultSearchItem, VaultTree,
};
use crate::model::platform::SearchResults;
use crate::output::Out;

/// `bcode docs [path] [--list] [--write-file f] [--search kw]`：缺省目录树；
/// 给 path 读正文；`--write-file` 写入（写前自动 .history 快照）；
/// `--search` vault 内搜索。v4 起全部走平台免参别名（/agent/docs/*，
/// 会话推导项目，X-Session 即上下文）。
pub async fn docs(cfg: &Config, profile: &str, a: &crate::cli::DocsArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;

    // vault 内搜索（免参别名）
    if let Some(kw) = &a.search {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct DocsSearchRes {
            #[serde(default, deserialize_with = "crate::model::null_to_default")]
            list: Vec<VaultSearchItem>,
        }
        let res: DocsSearchRes = ctx
            .sessioned_get(&format!(
                "/v1/agent/docs/search?keyword={}",
                encode_query(kw)
            ))
            .await?;
        if res.list.is_empty() {
            out.line("（vault 内无命中）");
        }
        for it in &res.list {
            out.line(&format!("  {} {}", it.path, it.title));
        }
        out.emit_value(&serde_json::to_value(&res)?);
        return Ok(());
    }

    // 写通道：本地文件 → vault 路径（免参别名）
    if let Some(local) = &a.write_file {
        let target = a.path.as_deref().ok_or_else(|| {
            anyhow!(
                "docs --write-file 需要目标路径：bcode docs <vault 路径> --write-file <本地文件>"
            )
        })?;
        let content =
            std::fs::read_to_string(local).with_context(|| format!("读取本地文件 {local} 失败"))?;
        let res: DocWriteResult = ctx
            .client
            .put_as(
                "/v1/agent/docs/file",
                json!({ "path": target, "content": content }),
            )
            .await?;
        out.kv(
            "已写入",
            &format!(
                "{}（{} 字节，写前已自动 .history 快照）",
                res.path, res.size
            ),
        );
        out.emit_value(&serde_json::to_value(&res)?);
        return Ok(());
    }

    match &a.path {
        None => {
            let tree: VaultTree = ctx.sessioned_get("/v1/agent/docs/tree").await?;
            if tree.tree.is_empty() {
                out.line("（文档树为空）");
            }
            if a.list {
                for n in flatten_tree(&tree.tree) {
                    if !n.is_dir {
                        out.line(&n.path);
                    }
                }
                let files: Vec<&str> = flatten_tree(&tree.tree)
                    .iter()
                    .filter(|n| !n.is_dir)
                    .map(|n| n.path.as_str())
                    .collect();
                out.emit_value(&json!({ "files": files }));
            } else {
                render_tree(&tree.tree, 0, out);
                out.emit_value(&serde_json::to_value(&tree)?);
            }
        }
        Some(p) => {
            let file: DocFile = ctx
                .sessioned_get(&format!("/v1/agent/docs/file?path={}", encode_query(p)))
                .await?;
            if file.binary {
                bail!(
                    "「{p}」是二进制文件（{} 字节），CLI 仅支持文本读取",
                    file.size
                );
            }
            // human 模式直接输出正文，便于管道与 agent 消费
            if !out.json {
                print!("{}", file.content);
            }
            out.emit_value(&serde_json::to_value(&file)?);
        }
    }
    Ok(())
}

/// `bcode memories [--prefix p]`（F16）：项目记忆列表（agent 间经验传递载体）。
pub async fn memories(cfg: &Config, profile: &str, prefix: Option<&str>, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;
    let mut path = format!("/v1/projects/{pid}/memories");
    if let Some(p) = prefix {
        path.push_str(&format!("?prefix={}", encode_query(p)));
    }
    let list: MemoryList = ctx.client.get_as(&path).await?;

    if list.list.is_empty() {
        out.line("（无记忆）");
    }
    for m in &list.list {
        out.line(&format!(
            "  [{:<8}] {} = {}",
            m.status,
            m.key,
            preview(&m.value)
        ));
    }
    out.emit_value(&serde_json::to_value(&list)?);
    Ok(())
}

/// `bcode memory <key>`（F16 + v2 写侧扩展）：
/// 缺省读取（human 直出 value）；`--set/--file` 写入（upsert）；
/// `--delete` 删除。记忆是 agent 间经验传递的载体，写通道补齐后
/// 「踩坑→沉淀为记忆→下一个 agent 消费」闭环成立。
pub async fn memory(cfg: &Config, profile: &str, a: &MemoryArgs, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;
    let key_path = format!("/v1/projects/{pid}/memories/{}", encode_query(&a.key));

    if a.delete {
        ctx.client.delete(&key_path).await?;
        out.kv("已删除", &a.key);
        out.emit_value(&json!({ "deleted": true, "key": a.key }));
        return Ok(());
    }

    if a.set.is_some() || a.file.is_some() {
        let value = match (a.file.as_deref(), a.set.as_deref()) {
            (Some(f), _) => std::fs::read_to_string(f).with_context(|| format!("读取 {f} 失败"))?,
            (None, Some(s)) => s.to_string(),
            (None, None) => unreachable!(),
        };
        let mut body = json!({ "value": value });
        if let Some(ttl) = a.ttl.as_deref() {
            body["ttl"] = json!(ttl);
        }
        ctx.client.put(&key_path, body).await?;
        out.kv("已写入", &a.key);
        out.emit_value(&json!({ "set": true, "key": a.key }));
        return Ok(());
    }

    // 读取
    let item: MemoryItem = ctx.client.get_as(&key_path).await?;
    if !out.json {
        println!("{}", item.value);
    }
    out.emit_value(&serde_json::to_value(&item)?);
    Ok(())
}

/// `bcode search <kw>`（F17）：全局搜索（agent 已获读权的项目范围）。
pub async fn search(
    cfg: &Config,
    profile: &str,
    query: &str,
    module: Option<&str>,
    out: &Out,
) -> Result<()> {
    let client = identity_client(cfg, profile)?;
    let mut path = format!("/v1/search?q={}", encode_query(query));
    if let Some(m) = module {
        path.push_str(&format!("&module={}", encode_query(m)));
    }
    let res: SearchResults = client.get_as(&path).await?;

    if res.list.is_empty() {
        out.line(&format!("（无结果，共 {} 条记录）", res.total));
    }
    for r in &res.list {
        out.line(&format!("  [{:<10}] #{:<5} {}", r.module, r.id, r.title));
        if !r.summary.is_empty() {
            out.line(&format!("      {}", preview(&r.summary)));
        }
    }
    out.emit_value(&serde_json::to_value(&res)?);
    Ok(())
}

/// 深度优先展平目录树（--list 与 json files 共用）
fn flatten_tree(nodes: &[VaultNode]) -> Vec<&VaultNode> {
    let mut out = vec![];
    for n in nodes {
        out.push(n);
        out.extend(flatten_tree(&n.children));
    }
    out
}

/// 缩进渲染目录树
fn render_tree(nodes: &[VaultNode], depth: usize, out: &Out) {
    for n in nodes {
        let suffix = if n.is_dir { "/" } else { "" };
        out.line(&format!("{}{}{}", "  ".repeat(depth), n.name, suffix));
        if n.is_dir {
            render_tree(&n.children, depth + 1, out);
        }
    }
}

/// 单行预览：首行截断
fn preview(s: &str) -> String {
    let first = s.lines().next().unwrap_or("");
    let mut cut: String = first.chars().take(60).collect();
    if first.chars().count() > 60 || s.lines().count() > 1 {
        cut.push('…');
    }
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, path: &str, is_dir: bool, children: Vec<VaultNode>) -> VaultNode {
        VaultNode {
            name: name.into(),
            path: path.into(),
            is_dir,
            size: 0,
            mod_time: String::new(),
            children,
        }
    }

    #[test]
    fn flatten_tree_walks_depth_first() {
        let tree = vec![
            node(
                "a",
                "a",
                true,
                vec![node("f1.md", "a/f1.md", false, vec![])],
            ),
            node("b.md", "b.md", false, vec![]),
        ];
        let flat: Vec<&str> = flatten_tree(&tree)
            .iter()
            .map(|n| n.path.as_str())
            .collect();
        assert_eq!(flat, vec!["a", "a/f1.md", "b.md"]);
    }

    #[test]
    fn preview_truncates_long_first_line() {
        assert_eq!(preview("短句"), "短句");
        assert_eq!(preview("多\n行"), "多…");
        let long: String = "x".repeat(80);
        assert!(preview(&long).ends_with('…'));
    }
}
