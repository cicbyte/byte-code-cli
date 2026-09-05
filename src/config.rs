//! ~/.bc/ 全局布局（协议 4A：身份在主目录，项目目录只存指向）：
//!   ~/.bc/config.toml                    server_url / default_profile
//!   ~/.bc/agents/<profile>/credential    { name, agent_id, api_key }
//!   ~/.bc/sessions/<profile>/<pid>.json  { session_id, project_id, project_name }
//!   <repo>/.bc/project                   { project_id, project_name }（可进 git，无身份）
//! BC_HOME 环境变量可覆盖 ~/.bc 根——测试与 CI 指向临时目录，不污染真实主目录。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cred;

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// 平台地址，含 API 前缀，如 http://127.0.0.1:8000/api
    pub server_url: Option<String>,
    pub default_profile: Option<String>,
}

pub fn bc_root() -> PathBuf {
    if let Ok(root) = std::env::var("BC_HOME") {
        let root = root.trim();
        if !root.is_empty() {
            return PathBuf::from(root);
        }
    }
    dirs::home_dir().expect("无法定位用户主目录").join(".bc")
}

pub fn config_path() -> PathBuf {
    bc_root().join("config.toml")
}

pub fn load_config() -> Result<Config> {
    let p = config_path();
    if !p.exists() {
        return Ok(Config::default());
    }
    let raw = fs::read_to_string(&p).with_context(|| format!("读取 {} 失败", p.display()))?;
    toml::from_str(&raw).with_context(|| format!("解析 {} 失败", p.display()))
}

// M1：register/join 流程写回配置时接入
#[allow(dead_code)]
pub fn save_config(cfg: &Config) -> Result<()> {
    let p = config_path();
    fs::create_dir_all(bc_root()).with_context(|| "创建 ~/.bc 失败")?;
    let raw = toml::to_string_pretty(cfg)?;
    fs::write(&p, raw).with_context(|| format!("写入 {} 失败", p.display()))?;
    Ok(())
}

/// 生效配置：server_url 未配置时报错并给出引导（F18 的 init 交互留 M3，
/// 当前阶段直接提示手写 config.toml）
pub fn effective_server_url(cfg: &Config) -> Result<String> {
    match &cfg.server_url {
        Some(u) => Ok(u.trim_end_matches('/').to_string()),
        None => bail!(
            "未配置平台地址：请在 {} 写入 server_url = \"http://<host>:8000/api\"",
            config_path().display()
        ),
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

/// 向上查找 .bc/project（当前目录及祖先——agent 可能在 repo 子目录工作）
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
        cur = dir.parent();
    }
    Ok(None)
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
}
