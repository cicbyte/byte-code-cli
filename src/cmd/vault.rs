//! 上下文消费域命令（F15-F17）：docs 文档中枢 / memory·memories 项目记忆 /
//! search 全局搜索。

use anyhow::{Result, bail};
use serde_json::json;

use super::{identity_client, project_ctx};
use crate::client::encode_query;
use crate::config::Config;
use crate::model::docs::{DocFile, MemoryItem, MemoryList, VaultNode, VaultTree};
use crate::model::platform::SearchResults;
use crate::output::Out;

/// `bcode docs [path] [--list]`（F15）：缺省展示目录树；给 path 输出文件正文。
/// 平台无免参别名，从 .bc/project 取项目 id 调 /v1/projects/{id}/docs/*。
pub async fn docs(
    cfg: &Config,
    profile: &str,
    path: Option<&str>,
    list: bool,
    out: &Out,
) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;

    match path {
        None => {
            let tree: VaultTree = ctx
                .client
                .get_as(&format!("/v1/projects/{pid}/docs/tree"))
                .await?;
            if tree.tree.is_empty() {
                out.line("（文档树为空）");
            }
            if list {
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
                .client
                .get_as(&format!(
                    "/v1/projects/{pid}/docs/file?path={}",
                    encode_query(p)
                ))
                .await?;
            if file.binary {
                bail!(
                    "「{}」是二进制文件（{} 字节），CLI 仅支持文本读取",
                    p,
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

/// `bcode memory <key>`（F16）：读取单条记忆——human 模式直接输出 value。
pub async fn memory(cfg: &Config, profile: &str, key: &str, out: &Out) -> Result<()> {
    let ctx = project_ctx(cfg, profile).await?;
    let pid = ctx.project.project_id;
    let item: MemoryItem = ctx
        .client
        .get_as(&format!(
            "/v1/projects/{pid}/memories/{}",
            encode_query(key)
        ))
        .await?;
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
