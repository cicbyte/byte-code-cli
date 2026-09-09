//! 任务 / 评论 / AI 执行日志端点类型（源：`api/v1/project/project.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// 创建类响应 {id}（评论 / 执行日志）
#[derive(Debug, Deserialize)]
pub struct CreatedId {
    #[serde(default)]
    pub id: i64,
}

/// GET /v1/tasks/{id} 响应（TaskItem 的 CLI 所需子集，未知字段忽略）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetail {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub project_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub assignee_id: i64,
    #[serde(default)]
    pub assignee_name: String,
    #[serde(default)]
    pub creator_name: String,
    #[serde(default)]
    pub artifacts: String,
    /// 步骤清单 JSON 字符串 [{text,done}]——长任务工作流的进度载体（打勾=续租约）
    #[serde(default)]
    pub checklist: String,
    /// 直接子任务（父子关系回填，v4 平台新增）
    #[serde(default, deserialize_with = "super::null_to_default")]
    pub sub_tasks: Vec<TaskBriefRef>,
    #[serde(default)]
    pub due_date: String,
    #[serde(default, deserialize_with = "null_to_default")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

impl TaskDetail {
    pub fn one_line(&self) -> String {
        format!("#{} [{}] {}", self.id, self.status, self.title)
    }
}

/// GET /v1/tasks/{taskId}/comments 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct CommentList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<CommentItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub real_name: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub created_at: String,
}

/// GET /v1/tasks/{taskId}/ai-logs 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct AiLogList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<AiLogItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiLogItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub ai_username: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub created_at: String,
}

/// TaskDetail.subTasks 元素（TaskItem 精简视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskBriefRef {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: i64,
}
