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
    #[serde(default)]
    pub project_code: String,
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
    /// 待分析跨项目反馈（阅读后 convert 建任务或 dismiss）
    #[serde(default, deserialize_with = "null_to_default")]
    pub pending_feedbacks: Vec<FeedbackBrief>,
    /// 分配给本 agent 的进行中专题
    #[serde(default, deserialize_with = "null_to_default")]
    pub active_topics: Vec<TopicBrief>,
    /// 高频 QA（按命中数前 5——遇到问题先查 QA 库再问人）
    #[serde(default, deserialize_with = "null_to_default")]
    pub top_qas: Vec<QaBrief>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectBrief {
    pub id: i64,
    #[serde(default)]
    pub code: String,
    pub name: String,
}

/// 跨项目反馈摘要（开工包）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackBrief {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub source_project_name: String,
    #[serde(default)]
    pub source_task_id: i64,
}

/// 专题摘要（开工包）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopicBrief {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub doc_path: String,
    #[serde(default)]
    pub phase_total: i64,
    #[serde(default)]
    pub phase_done: i64,
    #[serde(default)]
    pub last_handoff: String,
}

/// QA 摘要（开工包 TopQas）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaBrief {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub hits: i64,
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

/// 任务摘要（开工包与免参列表共用；v3 起平台补 type/tags/updatedAt）
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
    pub r#type: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

impl TaskBrief {
    /// 单行摘要：#id [status] title
    pub fn one_line(&self) -> String {
        let typ = if self.r#type.is_empty() {
            String::new()
        } else {
            format!("{}/", self.r#type)
        };
        format!("#{} [{}{typ}] {}", self.id, self.status, self.title)
    }
}
