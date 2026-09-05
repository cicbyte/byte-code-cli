//! 平台侧通用端点类型：通知 / 全局搜索 / 项目列表
//! （源：`api/v1/platform/platform.go`、`api/v1/project/project.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// GET /v1/notifications 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationList {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<NotificationItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub is_read: i64,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub source_id: i64,
    #[serde(default)]
    pub created_at: String,
}

impl NotificationItem {
    pub fn unread(&self) -> bool {
        self.is_read == 0
    }
}

/// GET /v1/search 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResults {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<SearchItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchItem {
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub project_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
}

/// GET /v1/projects 响应（start --project 名称解析用，取 id/name 子集；
/// agent 视角只含已获准入的项目）
#[derive(Debug, Deserialize)]
pub struct ProjectList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<ProjectRef>,
    #[serde(default)]
    pub total: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub name: String,
}
