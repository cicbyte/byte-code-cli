//! Agent 接入协议端点类型（源：byte-code `api/v1/agent/agent.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// POST /v1/agent/register 响应——key 只此一次返回，落盘后不再可见
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterCreated {
    pub agent_id: i64,
    pub api_key: String,
}

/// Debug 脱敏（与 cred::Credential 同款）：key 不进任何调试输出
impl std::fmt::Debug for RegisterCreated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisterCreated")
            .field("agent_id", &self.agent_id)
            .field("api_key", &"bc_***")
            .finish()
    }
}

/// POST /v1/agent/projects/join 响应
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinResult {
    pub project_id: i64,
    pub project_name: String,
}

/// POST /v1/agent/sessions 响应（开工包；键=agent+project，同键复用续期）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreated {
    pub session_id: String,
    pub project: ProjectBrief,
    #[serde(default, deserialize_with = "null_to_default")]
    pub conventions: Vec<ConventionItem>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub my_tasks: Vec<TaskBrief>,
    #[serde(default, deserialize_with = "null_to_default")]
    pub pending_reviews: Vec<TaskBrief>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrief {
    pub id: i64,
    pub name: String,
}

/// 约定条目（scope = global / project，项目侧优先）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConventionItem {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub scope: String,
}

/// GET /v1/agent/tasks 响应（会话免参推导项目）
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentTasks {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<TaskBrief>,
}

/// 任务摘要（开工包与免参列表共用）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskBrief {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub due_date: Option<String>,
}

impl TaskBrief {
    /// 单行摘要：#id [status] title
    pub fn one_line(&self) -> String {
        format!("#{} [{}] {}", self.id, self.status, self.title)
    }
}
