//! 文档中枢（vault）与项目记忆端点类型（源：`api/v1/docs/docs.go`）。

use serde::{Deserialize, Serialize};

/// GET /v1/projects/{id}/docs/tree 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultTree {
    #[serde(default)]
    pub tree: Vec<VaultNode>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultNode {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub mod_time: String,
    #[serde(default)]
    pub children: Vec<VaultNode>,
}

/// GET /v1/projects/{id}/docs/file 响应（文本返回正文；二进制仅元数据）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocFile {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub binary: bool,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub size: i64,
}

/// GET /v1/projects/{id}/memories 响应（单条记忆为同构内嵌）
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryList {
    #[serde(default)]
    pub list: Vec<MemoryItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItem {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub updated_at: String,
}
