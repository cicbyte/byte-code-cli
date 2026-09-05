//! 便利与生态命令（F18-F20）：init 引导配置 / open Web 深链 /
//! completion shell 补全 / man 手册。

use anyhow::{Result, anyhow, bail};
use serde_json::json;
use std::io::{BufRead, IsTerminal, Write};

use crate::cli::{Cli, OpenTarget};
use crate::client::BcodeClient;
use crate::config::{self, Config};
use crate::error::BcodeError;
use crate::output::Out;

/// `bcode init [server_url]`（F18）：引导式写 config.toml。
/// 参数缺省且 stdin 为 TTY 时交互询问；非交互环境（CI/--json）必须直接传参。
/// 保存前做连通性探测（任何 HTTP 响应含 401 均视为可达），不可达仅警告不阻断。
pub async fn init(cfg: &mut Config, url: Option<&str>, out: &Out) -> Result<()> {
    let server = match url {
        Some(u) => u.trim().trim_end_matches('/').to_string(),
        None => {
            if !std::io::stdin().is_terminal() {
                bail!(
                    "非交互环境请直接传参：bcode init <server_url>（如 http://127.0.0.1:8000/api）"
                );
            }
            prompt("平台地址（含 /api 前缀）", "http://127.0.0.1:8000/api")?
        }
    };
    if !server.starts_with("http://") && !server.starts_with("https://") {
        bail!("地址需以 http:// 或 https:// 开头：{server}");
    }

    let reachable = match BcodeClient::anonymous(server.clone(), cfg.insecure)?
        .get("/v1/health")
        .await
    {
        Ok(_) => true,
        // Auth/Business 都来自平台的 HTTP 响应壳——能收到即证明可达
        Err(e) => !matches!(e.downcast_ref::<BcodeError>(), Some(BcodeError::Network(_))),
    };
    if !reachable {
        eprintln!("bcode: 警告：{server} 暂不可达（仍已保存，可用 bcode status 复验）");
    }

    cfg.server_url = Some(server);
    config::save_config(cfg)?;

    let path = config::config_path()?;
    out.kv("server", cfg.server_url.as_deref().unwrap_or(""));
    out.kv("已写入", &path.display().to_string());
    out.emit_value(&json!({
        "server_url": cfg.server_url,
        "config_path": path.display().to_string(),
    }));
    Ok(())
}

/// `bcode open <task|board>`（F19）：Web 深链——URL 打印到 stdout 并尝试唤起浏览器。
/// 前端暂无任务直达路由（详情为列表页弹窗），task 目标暂落项目看板。
pub fn open(cfg: &Config, target: &OpenTarget, id: Option<i64>, out: &Out) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let base = server.trim_end_matches('/');
    let web = base.strip_suffix("/api").unwrap_or(base).to_string();

    let cwd = std::env::current_dir()?;
    let ptr = config::find_project_pointer(&cwd)?.ok_or_else(|| {
        anyhow!("当前目录不在任何项目内：open 需要项目上下文（在项目仓库内执行或先 bcode join）")
    })?;

    let mut note = "";
    let url = match target {
        OpenTarget::Board => format!("{web}/project/{}/board", ptr.project_id),
        OpenTarget::Task => {
            if id.is_none() {
                bail!("open task 需要任务 id：bcode open task 42");
            }
            note = "（前端暂无任务直达路由，已打开项目看板，从列表进入详情）";
            format!("{web}/project/{}/board", ptr.project_id)
        }
    };

    out.kv("URL", &url);
    if !note.is_empty() {
        out.line(note);
    }
    open_browser(&url);
    out.emit_value(&json!({ "url": url, "project_id": ptr.project_id }));
    Ok(())
}

/// `bcode completion <shell>`（F20）：补全脚本直出 stdout（自行 source 或安装到
/// 补全目录）；不受 --json 影响——产物本身就是脚本。
pub fn completion(shell: clap_complete::Shell) -> Result<()> {
    use clap::CommandFactory;
    use clap_complete::generate;
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "bcode", &mut std::io::stdout());
    Ok(())
}

/// `bcode man`（F20）：roff 手册直出 stdout（重定向到 man 目录使用）。
pub fn man() -> Result<()> {
    use clap::CommandFactory;
    let man = clap_mangen::Man::new(Cli::command());
    man.render(&mut std::io::stdout())?;
    Ok(())
}

/// TTY 询问：回车取默认值（提示走 stderr，不污染 --json 的 stdout）
fn prompt(label: &str, default: &str) -> Result<String> {
    eprint!("{label} [{default}]：");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    let t = line.trim();
    Ok(if t.is_empty() {
        default.to_string()
    } else {
        t.to_string()
    })
}

/// 跨平台唤起浏览器；失败静默（URL 已打印，用户可手动打开）
fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let spawned = std::process::Command::new("xdg-open").arg(url).spawn();
    let _ = spawned.map(|_| ()); // spawn 失败不影响命令结果
}
