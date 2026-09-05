//! 错误体系与退出码（需求 F22）：
//! 0 成功 / 2 用法错 / 3 认证失效 / 4 权限拒 / 5 网络错 / 6 业务拒。
//! 供编排脚本按码判定，错误信息走 stderr，数据走 stdout。
//! 响应壳类型（ApiEnvelope）在 model 层，此处只管错误分类与出口码。

#[derive(Debug, thiserror::Error)]
pub enum BcodeError {
    /// 认证失效：key 无效/被禁用/会话过期——提示重新 join 或 start
    #[error("认证失效：{0}（尝试 bcode register / bcode start）")]
    Auth(String),
    /// 权限拒绝：未获项目准入等
    #[error("权限拒绝：{0}（向项目 owner 索取接入码后 bcode join）")]
    Forbidden(String),
    /// 网络/服务不可达
    #[error("网络错误：{0}")]
    Network(String),
    /// 业务拒绝：认领被抢/名称已存在等
    #[error("{0}")]
    Business(String),
}

impl BcodeError {
    pub fn exit_code(&self) -> u8 {
        match self {
            BcodeError::Auth(_) => 3,
            BcodeError::Forbidden(_) => 4,
            BcodeError::Network(_) => 5,
            BcodeError::Business(_) => 6,
        }
    }
}

/// 顶层出口：anyhow 链中提取 BcodeError 用其退出码，其余归为用法/内部错
pub fn exit_code_of(err: &anyhow::Error) -> u8 {
    match err.downcast_ref::<BcodeError>() {
        Some(e) => e.exit_code(),
        None => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_contract() {
        assert_eq!(BcodeError::Auth("x".into()).exit_code(), 3);
        assert_eq!(BcodeError::Forbidden("x".into()).exit_code(), 4);
        assert_eq!(BcodeError::Network("x".into()).exit_code(), 5);
        assert_eq!(BcodeError::Business("x".into()).exit_code(), 6);
    }

    #[test]
    fn exit_code_of_downcasts_otherwise_usage() {
        let auth: anyhow::Error = BcodeError::Auth("key 失效".into()).into();
        assert_eq!(exit_code_of(&auth), 3);
        assert_eq!(exit_code_of(&anyhow::anyhow!("参数缺失")), 2);
    }
}
