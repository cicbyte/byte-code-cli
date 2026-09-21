//! 工作日志端点类型（源：`api/v1/project/worklog.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// GET /v1/projects/{pid}/worklogs 响应（倒序分页）
#[derive(Debug, Serialize, Deserialize)]
pub struct WorklogList {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<WorklogItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorklogItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub author_name: String,
    /// human / ai
    #[serde(default)]
    pub author_type: String,
    #[serde(default)]
    pub content: String,
    /// manual 手写 / tasks 草稿生成
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub created_at: String,
}

/// GET /v1/projects/{pid}/worklogs/draft 响应（不落库的草稿文本）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorklogDraft {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub task_count: i64,
    #[serde(default)]
    pub release_count: i64,
}
