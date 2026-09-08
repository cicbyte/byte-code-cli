//! 专题端点类型（源：`api/v1/project/topic.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// GET /v1/projects/{id}/topics 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TopicList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<TopicItem>,
}

/// GET /v1/projects/{id}/topics/{id} 响应（Go 内嵌 TopicItem——字段平铺，flatten 还原）
#[derive(Debug, Serialize, Deserialize)]
pub struct TopicDetailRes {
    #[serde(flatten)]
    pub topic: TopicItem,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub acceptance: String,
    #[serde(default)]
    pub doc_path: String,
    #[serde(default)]
    pub assignee_name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub phase_total: i64,
    #[serde(default)]
    pub phase_done: i64,
    /// 最近一次交接摘要（下个会话恢复点）
    #[serde(default)]
    pub last_handoff: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub phases: Vec<TopicPhaseBrief>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicPhaseBrief {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub status: String,
    /// 已转出任务的阶段回填（0=未转）
    #[serde(default)]
    pub task_id: i64,
}
