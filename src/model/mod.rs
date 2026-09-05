//! 平台 API 线缆类型的 serde 强类型，按端点域分文件
//! （源：byte-code `api/v1/{agent,project,platform,docs}`，字段为 camelCase）。
//! 反序列化即校验，替代命令内裸 `Value` 拼装。

pub mod agent;
pub mod docs;
pub mod platform;
pub mod task;

use serde::Deserialize;

use crate::error::BcodeError;

/// Go nil slice 会序列化为显式 null（而非省略键）——
/// 此助手把 null 容错为空集合，配合 #[serde(default)] 覆盖键缺席与 null 两态
pub fn null_to_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    T: Default + Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let v = Option::<T>::deserialize(d)?;
    Ok(v.unwrap_or_default())
}

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

    #[test]
    fn go_nil_slice_null_decodes_to_empty() {
        // 真机形态：Go nil slice 序列化为显式 null（2026-09-05 平台实测）
        let v = serde_json::json!({
            "sessionId": "bcsh_2", "project": { "id": 4, "name": "byte-code-cli" },
            "conventions": null, "myTasks": null, "pendingReviews": null
        });
        let boot: agent::SessionCreated = serde_json::from_value(v).unwrap();
        assert!(boot.conventions.is_empty());
        assert!(boot.my_tasks.is_empty());
        assert!(boot.pending_reviews.is_empty());
    }
}
