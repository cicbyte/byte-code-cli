//! 平台 API 线缆类型的 serde 强类型（源：byte-code `api/v1/agent/agent.go`，
//! 字段为 camelCase）。反序列化即校验，替代命令内裸 `Value` 拼装；
//! M1 各端点（register/join/sessions/claim…）在此扩充。

use serde::Deserialize;

use crate::error::BcodeError;

/// 平台统一响应壳 {code, message, data}（GoFrame 标准格式）
#[derive(Debug, Deserialize)]
pub struct ApiEnvelope {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

impl ApiEnvelope {
    /// code=0 成功；非 0 时按语义分类（平台 code 50=通用业务错，401/403 同名映射）
    pub fn into_data(self) -> Result<serde_json::Value, BcodeError> {
        if self.code == 0 {
            Ok(self.data)
        } else if self.message.contains("未登录") || self.message.contains("认证") {
            Err(BcodeError::Auth(self.message))
        } else if self.message.contains("无权限") || self.message.contains("权限") {
            Err(BcodeError::Forbidden(self.message))
        } else {
            Err(BcodeError::Business(self.message))
        }
    }
}

/// GET /v1/agent/tasks 响应（status 采样可见任务用）
#[derive(Debug, Deserialize)]
pub struct TaskList {
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub list: Vec<TaskBrief>,
}

/// 任务摘要（工作会话开工包与任务列表共用）
#[derive(Debug, Deserialize)]
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
    /// 仅有值时平台才输出
    #[serde(default)]
    pub due_date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_zero_code_passes_data_through() {
        let env = ApiEnvelope {
            code: 0,
            message: "OK".into(),
            data: serde_json::json!({ "x": 1 }),
        };
        assert_eq!(env.into_data().unwrap(), serde_json::json!({ "x": 1 }));
    }

    #[test]
    fn envelope_classifies_auth_forbidden_business() {
        let mk = |message: &str| ApiEnvelope {
            code: 50,
            message: message.into(),
            data: serde_json::Value::Null,
        };
        assert!(matches!(
            mk("用户未登录").into_data(),
            Err(BcodeError::Auth(_))
        ));
        assert!(matches!(
            mk("无权限访问").into_data(),
            Err(BcodeError::Forbidden(_))
        ));
        assert!(matches!(
            mk("任务已被认领").into_data(),
            Err(BcodeError::Business(_))
        ));
    }

    #[test]
    fn task_list_decodes_wire_payload() {
        let v = serde_json::json!({
            "total": 3,
            "list": [{ "id": 1, "title": "t", "status": "open", "priority": 2, "dueDate": "2026-01-01" }]
        });
        let tl: TaskList = serde_json::from_value(v).unwrap();
        assert_eq!(tl.total, 3);
        assert_eq!(tl.list.len(), 1);
        assert_eq!(tl.list[0].due_date.as_deref(), Some("2026-01-01"));
    }
}
