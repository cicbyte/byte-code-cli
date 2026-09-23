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
    /// 注册实例地址（多实例防串：这个身份属于哪套平台——配合平台侧
    /// register_ip 交叉定位归属；旧凭证无此键时空串兼容）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server_url: String,
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
    /// 项目级服务器地址（多实例部署：公司/个人各一套时，本 repo 绑定特定实例）。
    /// 空=用全局 config.toml；非空优先于全局
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server_url: String,
}

/// 会话缓存：数据根下 sessions/<profile>/<project_id>.json
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub project_id: i64,
    pub project_name: String,
    /// 建立会话时的服务器地址——load 时与当前生效地址不匹配则视为失效
    /// （同一 profile 同一 project id 在不同实例各有会话，防串）
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub server_url: String,
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

/// 会话路径按实例隔离：sessions/<profile>/<host_port>/<pid>.json——
/// project_id 跨实例不唯一（公司/个人实例各有 id=N 的项目），同 profile
/// 双实例的会话必须可共存，否则来回切换互相覆盖、每次都重建。
/// 实例目录取 server 的 host[:port]（冒号转下划线，可读可调试）；
/// 旧两段路径（无实例层）自然失配走懒重建，无需迁移
pub fn session_path(profile: &str, server: &str, project_id: i64) -> Result<PathBuf> {
    let instance = instance_dir(server);
    Ok(bc_root()?
        .join("sessions")
        .join(profile)
        .join(instance)
        .join(format!("{project_id}.json")))
}

/// server → 实例目录段：取 authority（host[:port]），: → _
fn instance_dir(server: &str) -> String {
    let rest = server
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    let authority = rest.split('/').next().unwrap_or(rest);
    if authority.is_empty() {
        "default".into()
    } else {
        authority.replace(':', "_")
    }
}

pub fn load_session(profile: &str, server: &str, project_id: i64) -> Option<SessionRecord> {
    let p = session_path(profile, server, project_id).ok()?;
    let raw = fs::read_to_string(p).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_session(profile: &str, server: &str, rec: &SessionRecord) -> Result<()> {
    let p = session_path(profile, server, rec.project_id)?;
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
    fn instance_dir_extracts_authority() {
        assert_eq!(
            instance_dir("http://127.0.0.1:18020/api"),
            "127.0.0.1_18020"
        );
        assert_eq!(
            instance_dir("https://dx4600.link:18026/api"),
            "dx4600.link_18026"
        );
        assert_eq!(instance_dir("http://plain.host/api"), "plain.host");
        assert_eq!(instance_dir("junk"), "junk");
    }

    #[test]
    fn session_paths_isolate_by_instance() {
        let a = session_path("dev", "http://a.com:1/api", 8).unwrap();
        let b = session_path("dev", "http://b.com:2/api", 8).unwrap();
        assert_ne!(a, b, "同 profile 同 pid 跨实例必须分文件");
        let expect = format!("a.com_1{}8.json", std::path::MAIN_SEPARATOR);
        assert!(a.ends_with(&expect) || a.ends_with("a.com_1/8.json"));
    }

    #[test]
    fn debug_output_never_contains_plain_key() {
        let cred = Credential {
            name: "bcode-e2e".into(),
            agent_id: 25,
            api_key: "bc_deadbeefdeadbeef".into(),
            server_url: String::new(),
        };
        let dbg = format!("{cred:?}");
        assert!(!dbg.contains("deadbeef"), "Debug 输出泄漏了明文 key：{dbg}");
        assert!(dbg.contains("bc_***"));
    }
}
