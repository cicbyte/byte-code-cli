//! 应用数据目录布局（协议 4A：身份在主目录，项目目录只存指向）：
//!   ~/.cicbyte/apps/byte-code-cli/config.toml                    server_url / default_profile
//!   ~/.cicbyte/apps/byte-code-cli/agents/<profile>/credential    { name, agent_id, api_key }
//!   ~/.cicbyte/apps/byte-code-cli/sessions/<profile>/<pid>.json  { session_id, project_id, project_name }
//!   <repo>/.bc/project                                            { project_id, project_name }（可进 git，无身份）
//! BC_HOME 环境变量可覆盖数据根——测试与 CI 指向临时目录，不污染真实主目录。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};

use crate::cred;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// 平台地址，含 API 前缀，如 http://127.0.0.1:8000/api
    pub server_url: Option<String>,
    pub default_profile: Option<String>,
    /// 跳过 TLS 证书校验（自签/调试用；--insecure 旗标优先于此）
    #[serde(default)]
    pub insecure: bool,
}

/// 数据根：~/.cicbyte/apps/byte-code-cli（BC_HOME 可覆盖）。
/// 主目录不可定位时报错（由调用方收敛为退出码 2），不再 panic
pub fn bc_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("BC_HOME") {
        let root = root.trim();
        if !root.is_empty() {
            return Ok(PathBuf::from(root));
        }
    }
    dirs::home_dir()
        .map(|h| h.join(".cicbyte").join("apps").join("byte-code-cli"))
        .ok_or_else(|| {
            anyhow!("无法定位用户主目录（HOME/USERPROFILE 均缺失）：可用 BC_HOME 显式指定数据根")
        })
}

pub fn config_path() -> Result<PathBuf> {
    Ok(bc_root()?.join("config.toml"))
}

pub fn load_config() -> Result<Config> {
    let p = config_path()?;
    if !p.exists() {
        return Ok(Config::default());
    }
    let raw = fs::read_to_string(&p).with_context(|| format!("读取 {} 失败", p.display()))?;
    toml::from_str(&raw).with_context(|| format!("解析 {} 失败", p.display()))
}

// M3：init 引导式写配置时接入
#[allow(dead_code)]
pub fn save_config(cfg: &Config) -> Result<()> {
    let p = config_path()?;
    fs::create_dir_all(bc_root()?)
        .with_context(|| "创建数据目录 ~/.cicbyte/apps/byte-code-cli 失败")?;
    let raw = toml::to_string_pretty(cfg)?;
    fs::write(&p, raw).with_context(|| format!("写入 {} 失败", p.display()))?;
    Ok(())
}

/// 生效配置：server_url 未配置时报错并给出引导（F18 的 init 交互留 M3，
/// 当前阶段直接提示手写 config.toml）
pub fn effective_server_url(cfg: &Config) -> Result<String> {
    let hint = config_path()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    match &cfg.server_url {
        Some(u) => Ok(u.trim_end_matches('/').to_string()),
        None => bail!("未配置平台地址：请在 {hint} 写入 server_url = \"http://<host>:8000/api\""),
    }
}

/// 当前 profile 解析：--profile > BC_AGENT > config.default_profile > "default"
pub fn effective_profile(cfg: &Config, flag: Option<&String>) -> String {
    let env = std::env::var("BC_AGENT")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    resolve_profile(cfg, flag.map(|s| s.as_str()), env.as_deref())
}

/// 纯函数拆分（env 读取在外层）：单测直接覆盖优先级组合
fn resolve_profile(cfg: &Config, flag: Option<&str>, env: Option<&str>) -> String {
    if let Some(p) = flag {
        return p.to_string();
    }
    if let Some(p) = env {
        return p.to_string();
    }
    cfg.default_profile
        .clone()
        .unwrap_or_else(|| "default".into())
}

/// profile 名合法性：profile 会拼进本地路径（agents/<profile>/credential），
/// 仅允许字母/数字/`._-`（1-64 字符，不得以 . 开头）——拒绝路径分隔符等
/// 字符，防 --profile/BC_AGENT 值把读写引出数据目录
pub fn validate_profile_name(profile: &str) -> Result<()> {
    let ok = !profile.is_empty()
        && profile.len() <= 64
        && !profile.starts_with('.')
        && profile
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if !ok {
        bail!("profile 名不合法「{profile}」：仅允许字母/数字/._-，1-64 字符且不以 . 开头");
    }
    Ok(())
}

/// 向上查找 .bc/project（当前目录及祖先——agent 可能在 repo 子目录工作）。
/// 以 .git 为仓库边界：不越出仓库向上找，防父目录杂散指针静默劫持项目上下文
pub fn find_project_pointer(start: &Path) -> Result<Option<cred::ProjectPointer>> {
    let mut cur = Some(start);
    while let Some(dir) = cur {
        let marker = dir.join(".bc").join("project");
        if marker.is_file() {
            let raw = fs::read_to_string(&marker)
                .with_context(|| format!("读取 {} 失败", marker.display()))?;
            let ptr: cred::ProjectPointer = serde_json::from_str(&raw)
                .with_context(|| format!("解析 {} 失败", marker.display()))?;
            return Ok(Some(ptr));
        }
        if dir.join(".git").exists() {
            return Ok(None);
        }
        cur = dir.parent();
    }
    Ok(None)
}

/// 写项目指向到 <root>/.bc/project（join 成功后调用），返回落盘路径
pub fn save_project_pointer(root: &Path, ptr: &cred::ProjectPointer) -> Result<PathBuf> {
    let marker = root.join(".bc").join("project");
    if let Some(dir) = marker.parent() {
        fs::create_dir_all(dir).with_context(|| format!("创建 {} 失败", dir.display()))?;
    }
    let raw = serde_json::to_string_pretty(ptr)?;
    fs::write(&marker, raw).with_context(|| format!("写入 {} 失败", marker.display()))?;
    Ok(marker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_priority_flag_env_config_default() {
        let cfg = Config {
            default_profile: Some("from-config".into()),
            ..Default::default()
        };
        assert_eq!(resolve_profile(&cfg, Some("flag"), Some("env")), "flag");
        assert_eq!(resolve_profile(&cfg, None, Some("env")), "env");
        assert_eq!(resolve_profile(&cfg, None, None), "from-config");
        assert_eq!(resolve_profile(&Config::default(), None, None), "default");
    }

    #[test]
    fn profile_name_rejects_path_escape_and_odd_chars() {
        for ok in ["default", "codex-cli", "claude_code", "a1.2"] {
            assert!(validate_profile_name(ok).is_ok(), "{ok} 应合法");
        }
        for bad in [
            "",
            "../evil",
            "a/b",
            ".hidden",
            "a b",
            "窗",
            &"x".repeat(65),
        ] {
            assert!(validate_profile_name(bad).is_err(), "{bad} 应被拒绝");
        }
    }

    #[test]
    fn project_pointer_walks_up_ancestors() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".bc")).unwrap();
        fs::write(
            root.join(".bc").join("project"),
            r#"{"project_id":7,"project_name":"demo"}"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("a").join("b")).unwrap();

        let ptr = find_project_pointer(&root.join("a").join("b"))
            .unwrap()
            .unwrap();
        assert_eq!(ptr.project_id, 7);
        assert_eq!(ptr.project_name, "demo");
    }

    #[test]
    fn project_pointer_rejects_malformed_marker() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".bc")).unwrap();
        fs::write(root.join(".bc").join("project"), "not-json").unwrap();
        assert!(find_project_pointer(root).is_err());
    }

    #[test]
    fn project_pointer_stops_at_git_boundary() {
        // 仓库外层被投放杂散指针：repo（含 .git）内部查找不得越界命中
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".bc")).unwrap();
        fs::write(
            root.join(".bc").join("project"),
            r#"{"project_id":666,"project_name":"hijack"}"#,
        )
        .unwrap();
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("sub")).unwrap();

        assert!(find_project_pointer(&repo.join("sub")).unwrap().is_none());
        assert!(find_project_pointer(&repo).unwrap().is_none());
    }

    #[test]
    fn project_pointer_finds_marker_in_repo_root() {
        // 指针与 .git 同级（正常形态：join 写在仓库根）必须能找到
        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join(".bc")).unwrap();
        fs::write(
            repo.join(".bc").join("project"),
            r#"{"project_id":7,"project_name":"demo"}"#,
        )
        .unwrap();
        fs::create_dir_all(repo.join("a")).unwrap();

        let ptr = find_project_pointer(&repo.join("a")).unwrap().unwrap();
        assert_eq!(ptr.project_id, 7);
    }
}
