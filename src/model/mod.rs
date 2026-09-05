//! 平台 API 线缆类型的 serde 强类型，按端点域分文件
//! （源：byte-code `api/v1/{agent,project,platform,docs}`，字段为 camelCase）。
//! 反序列化即校验，替代命令内裸 `Value` 拼装。

pub mod agent;
pub mod docs;
pub mod platform;
pub mod task;

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
    fn session_kickoff_decodes_camel_case() {
        let v = serde_json::json!({
            "sessionId": "bcsh_1", "project": { "id": 7, "name": "demo" },
            "conventions": [{ "key": "git.style", "value": "sq", "scope": "global" }]
        });
        let boot: agent::SessionCreated = serde_json::from_value(v).unwrap();
        assert_eq!(boot.session_id, "bcsh_1");
        assert_eq!(boot.project.id, 7);
        assert_eq!(boot.conventions[0].key, "git.style");
        // myTasks / pendingReviews 缺席时容错为空
        assert!(boot.my_tasks.is_empty());
        assert!(boot.pending_reviews.is_empty());
    }
}
