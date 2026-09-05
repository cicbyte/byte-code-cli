//! 身份域命令（注册层）：whoami 本地概览 / profiles 本地清单。

use anyhow::Result;
use serde_json::json;

use crate::config::{self, Config};
use crate::cred;
use crate::output::Out;

/// `bcode whoami`：纯本地——当前 profile/凭证/项目指向/会话缓存一屏概览。
/// 不打网络（离线可用），远端有效性校验是 status 的职责。
pub async fn whoami(cfg: &Config, profile: &str, out: &Out) -> Result<()> {
    let mut payload = json!({ "profile": profile });

    out.line(&format!("profile: {profile}"));

    match cred::load_credential(profile) {
        Ok(c) => {
            out.kv("agent", &c.name);
            out.kv("agent_id", &c.agent_id.to_string());
            payload["agent"] = json!(c.name);
            payload["agent_id"] = json!(c.agent_id);
            payload["has_credential"] = json!(true);
        }
        Err(_) => {
            out.kv("agent", "（无凭证——bcode register <name>）");
            payload["has_credential"] = json!(false);
        }
    }

    match &cfg.server_url {
        Some(u) => {
            out.kv("server", u);
            payload["server"] = json!(u);
        }
        None => {
            out.kv(
                "server",
                "（未配置——见 ~/.cicbyte/apps/byte-code-cli/config.toml）",
            );
            payload["server"] = json!(null);
        }
    }

    // 项目上下文：向上找 .bc/project
    let cwd = std::env::current_dir()?;
    if let Some(ptr) = config::find_project_pointer(&cwd)? {
        out.kv(
            "project",
            &format!("{} (id={})", ptr.project_name, ptr.project_id),
        );
        payload["project"] = json!({ "id": ptr.project_id, "name": ptr.project_name });
        if let Some(s) = cred::load_session(profile, ptr.project_id) {
            out.kv(
                "session",
                &format!("{}（缓存）", &s.session_id[..12.min(s.session_id.len())]),
            );
            payload["session"] = json!(s.session_id);
        } else {
            out.kv("session", "（未建立——bcode start）");
            payload["session"] = json!(null);
        }
    } else {
        out.kv("project", "（当前目录不在任何项目内——bcode join <code>）");
        payload["project"] = json!(null);
    }

    out.emit_value(&payload);
    Ok(())
}

/// `bcode profiles`：本地 profile 清单与 default 指向
pub fn profiles(cfg: &Config, out: &Out) -> Result<()> {
    let current = config::effective_profile(cfg, None);
    let names = cred::list_profiles();
    let mut arr = vec![];
    for n in &names {
        let is_cur = *n == current;
        let is_def = cfg.default_profile.as_deref() == Some(n.as_str());
        let mut marks = String::new();
        if is_cur {
            marks.push_str(" ←当前");
        }
        if is_def {
            marks.push_str(" ·default");
        }
        if marks.is_empty() {
            marks.push_str("   ");
        }
        let cred_name = cred::load_credential(n)
            .map(|c| c.name)
            .unwrap_or_else(|_| "?".into());
        out.line(&format!("  {n:<20} {cred_name}{marks}"));
        arr.push(json!({ "profile": n, "agent": cred_name, "current": is_cur }));
    }
    if names.is_empty() {
        out.line("（无 profile——bcode register <name> [--profile <p>] 创建）");
    }
    out.emit_value(&json!({ "profiles": arr, "current": current }));
    Ok(())
}
