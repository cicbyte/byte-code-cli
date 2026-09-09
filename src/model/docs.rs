//! 文档中枢（vault）与项目记忆端点类型（源：`api/v1/docs/docs.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// GET /v1/projects/{id}/docs/tree 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultTree {
    #[serde(default, deserialize_with = "null_to_default")]
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
    #[serde(default, deserialize_with = "null_to_default")]
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

/// PUT /v1/projects/{id}/docs/file 响应（写通道，v2 CLI 接入）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocWriteResult {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: i64,
}

/// vault 搜索命中项（GET /v1/agent/docs/search 免参别名，v4）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultSearchItem {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub space: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub tags: Vec<String>,
}

/// GET /v1/projects/{id}/memories 响应（单条记忆为同构内嵌）
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryList {
    #[serde(default, deserialize_with = "null_to_default")]
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
