//! 凭证与会话的本地存取（四A 布局的数据结构面）

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::bc_root;

/// 身份凭证：~/.bc/agents/<profile>/credential（0600，key 不回显）
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Credential {
    /// 平台侧名称（全局唯一，如 codex-cli）
    pub name: String,
    pub agent_id: i64,
    pub api_key: String,
}

/// 项目指向：<repo>/.bc/project（无身份信息，可进 git）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectPointer {
    pub project_id: i64,
    pub project_name: String,
}

/// 会话缓存：~/.bc/sessions/<profile>/<project_id>.json
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub project_id: i64,
    pub project_name: String,
}

pub fn credential_path(profile: &str) -> PathBuf {
    bc_root().join("agents").join(profile).join("credential")
}

pub fn load_credential(profile: &str) -> Result<Credential> {
    let p = credential_path(profile);
    let raw = fs::read_to_string(&p)
        .with_context(|| format!("profile「{profile}」无本地凭证（{}）", p.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("解析 {} 失败", p.display()))
}

/// 写凭证并在类 Unix 上收紧为 0600（Windows 依赖用户目录 ACL）
// M1：register 落盘凭证时接入
#[allow(dead_code)]
pub fn save_credential(profile: &str, cred: &Credential) -> Result<()> {
    let p = credential_path(profile);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    let raw = serde_json::to_string_pretty(cred)?;
    fs::write(&p, raw).with_context(|| format!("写入 {} 失败", p.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn list_profiles() -> Vec<String> {
    let dir = bc_root().join("agents");
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

pub fn session_path(profile: &str, project_id: i64) -> PathBuf {
    bc_root()
        .join("sessions")
        .join(profile)
        .join(format!("{project_id}.json"))
}

pub fn load_session(profile: &str, project_id: i64) -> Option<SessionRecord> {
    let p = session_path(profile, project_id);
    let raw = fs::read_to_string(p).ok()?;
    serde_json::from_str(&raw).ok()
}

// M1：start 建立会话后落盘
#[allow(dead_code)]
pub fn save_session(profile: &str, rec: &SessionRecord) -> Result<()> {
    let p = session_path(profile, rec.project_id);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&p, serde_json::to_string_pretty(rec)?)?;
    Ok(())
}
