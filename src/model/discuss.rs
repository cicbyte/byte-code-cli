//! 讨论区与项目发布（Releases）端点类型
//! （源：`api/v1/project/{discussion,release}.go`，随平台源码对齐）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

// ==================== 讨论区 ====================

/// GET /v1/projects/{pid}/discussions 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionList {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<DiscussionItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub body: String,
    /// open / converted / archived
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub author_name: String,
    /// human / ai（ai=agent 发起）
    #[serde(default)]
    pub author_type: String,
    #[serde(default)]
    pub reply_count: i64,
    #[serde(default)]
    pub updated_at: String,
}

/// GET /v1/discussions/{id} 响应（Go 内嵌平铺：详情含回复列表）
#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionDetail {
    #[serde(flatten)]
    pub d: DiscussionItem,
    #[serde(default, deserialize_with = "null_to_default")]
    pub replies: Vec<DiscussionReply>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionReply {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub user_name: String,
    #[serde(default)]
    pub user_type: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub created_at: String,
}

// ==================== 项目发布（Releases）====================

/// GET /v1/projects/{pid}/releases 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseList {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<ReleaseItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub notes: String,
    /// stable / beta / nightly
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub created_by_name: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub file_count: i64,
    #[serde(default)]
    pub total_size_bytes: i64,
}

/// GET /v1/releases/{id}/files 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseFileList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<ReleaseFileItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseFileItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub file_size: i64,
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub uploader_name: String,
    #[serde(default)]
    pub download_count: i64,
}
