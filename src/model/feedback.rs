//! 跨项目反馈与项目关联端点类型（源：`api/v1/project/feedback.go`、`relation.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// GET /v1/projects/{id}/feedbacks 响应（收件箱）
#[derive(Debug, Serialize, Deserialize)]
pub struct FeedbackList {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<FeedbackItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source_project_name: String,
    #[serde(default)]
    pub source_task_id: i64,
    /// convert 后回填的任务 id（0=未转）
    #[serde(default)]
    pub converted_task_id: i64,
    #[serde(default)]
    pub created_at: String,
}

/// GET /v1/projects/{id}/relations 响应（关联项目列表）
#[derive(Debug, Serialize, Deserialize)]
pub struct RelationList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<RelationItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationItem {
    #[serde(default)]
    pub id: i64,
    /// 关联对方项目 id
    #[serde(default)]
    pub project_id: i64,
    #[serde(default)]
    pub name: String,
}
