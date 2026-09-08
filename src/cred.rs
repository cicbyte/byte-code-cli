//! 凭证与会话的本地存取（四A 布局的数据结构面）

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::bc_root;

/// 身份凭证：数据根下 agents/<profile>/credential（0600，key 不回显）
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Credential {
    /// 平台侧名称（全局唯一，如 codex-cli）
    pub name: String,
    pub agent_id: i64,
    pub api_key: String,
}

/// Debug 脱敏：key 只保留前缀占位，杜绝任何 `{:?}` 调试输出泄漏
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credential")
            .field("name", &self.name)
            .field("agent_id", &self.agent_id)
            .field("api_key", &"bc_***")
            .finish()
    }
}

/// 项目指向：<repo>/.bc/project（无身份信息，可进 git）。
/// code 为项目短码（跨环境稳定，新平台字段）；旧指针无此字段时空串，
/// 会话建立自动退回 projectId——两代指针兼容
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectPointer {
    pub project_id: i64,
    #[serde(default)]
    pub project_code: String,
    pub project_name: String,
}

/// 会话缓存：数据根下 sessions/<profile>/<project_id>.json
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub project_id: i64,
    pub project_name: String,
}

pub fn credential_path(profile: &str) -> Result<PathBuf> {
    Ok(bc_root()?.join("agents").join(profile).join("credential"))
}

pub fn load_credential(profile: &str) -> Result<Credential> {
    let p = credential_path(profile)?;
    let raw = fs::read_to_string(&p)
        .with_context(|| format!("profile「{profile}」无本地凭证（{}）", p.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("解析 {} 失败", p.display()))
}

/// 落盘后收紧为仅属主可读（类 Unix 0600；Windows 依赖用户目录 ACL）
fn set_private(p: &std::path::Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(p, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = p;
    Ok(())
}

/// 写凭证并收紧权限（key 不回显）
pub fn save_credential(profile: &str, cred: &Credential) -> Result<()> {
    let p = credential_path(profile)?;
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    let raw = serde_json::to_string_pretty(cred)?;
    fs::write(&p, raw).with_context(|| format!("写入 {} 失败", p.display()))?;
    set_private(&p)?;
    Ok(())
}

pub fn list_profiles() -> Vec<String> {
    // 数据根不可定位（极罕见）时视同无 profile，不阻断本地命令
    let Ok(dir) = bc_root().map(|r| r.join("agents")) else {
        return vec![];
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };
    let mut out: Vec<String> = entries
        .flatten()
        .filter(|e| e.path().join("credential").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
}

pub fn session_path(profile: &str, project_id: i64) -> Result<PathBuf> {
    Ok(bc_root()?
        .join("sessions")
        .join(profile)
        .join(format!("{project_id}.json")))
}

pub fn load_session(profile: &str, project_id: i64) -> Option<SessionRecord> {
    let p = session_path(profile, project_id).ok()?;
    let raw = fs::read_to_string(p).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_session(profile: &str, rec: &SessionRecord) -> Result<()> {
    let p = session_path(profile, rec.project_id)?;
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&p, serde_json::to_string_pretty(rec)?)?;
    // 与凭证同款收紧：session_id 虽非权限边界，也不该同机可读
    set_private(&p)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_plain_key() {
        let cred = Credential {
            name: "bcode-e2e".into(),
            agent_id: 25,
            api_key: "bc_deadbeefdeadbeef".into(),
        };
        let dbg = format!("{cred:?}");
        assert!(!dbg.contains("deadbeef"), "Debug 输出泄漏了明文 key：{dbg}");
        assert!(dbg.contains("bc_***"));
    }
}
