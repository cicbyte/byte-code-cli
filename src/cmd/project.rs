//! 项目域命令（准入与会话层）：join 接入 / start 会话+开工包 /
//! context 约定输出 / projects 已加入清单 / status 在线校验。

use anyhow::{Result, anyhow, bail};
use serde_json::json;

use super::ensure_session;
use crate::client::BcodeClient;
use crate::config::{self, Config};
use crate::cred::{self, ProjectPointer};
use crate::model::agent::{AgentTasks, JoinResult, SessionCreated};
use crate::model::platform::ProjectList;
use crate::output::Out;

/// `bcode projects`（v2 反馈新增，v3 改走免参端点）：列出 agent 已接入的全部项目
/// （GET /v1/agent/projects，bc key 认证），标注当前目录指向——多项目切换入口。
pub async fn projects(cfg: &Config, profile: &str, out: &Out) -> Result<()> {
    let client = super::identity_client(cfg, profile)?;
    #[derive(serde::Deserialize)]
    struct AgentProjects {
        #[serde(default, deserialize_with = "crate::model::null_to_default")]
        list: Vec<crate::model::agent::ProjectBrief>,
    }
    let res: AgentProjects = client.get_as("/v1/agent/projects").await?;

    let cwd = std::env::current_dir()?;
    let current = config::find_project_pointer(&cwd)?.map(|p| p.project_id);

    if res.list.is_empty() {
        out.line("（未加入任何项目——向 owner 索取接入码后 bcode join <code>）");
    }
    let mut arr = vec![];
    for p in &res.list {
        let mark = if Some(p.id) == current {
            " ←当前目录"
        } else {
            ""
        };
        let code = if p.code.is_empty() {
            String::new()
        } else {
            format!(" code={}", p.code)
        };
        out.line(&format!("  #{:<5} {}{code}{mark}", p.id, p.name));
        arr.push(
            json!({ "id": p.id, "code": p.code, "name": p.name, "current": Some(p.id) == current }),
        );
    }
    out.emit_value(&json!({ "projects": arr }));
    Ok(())
}

/// `bcode join <code>`（F03）：接入码换项目准入，成功写 <repo>/.bc/project。
pub async fn join(cfg: &Config, profile: &str, code: &str, out: &Out) -> Result<()> {
    let client = super::identity_client(cfg, profile)?;
    let res: JoinResult = client
        .post_as("/v1/agent/projects/join", json!({ "code": code }))
        .await?;

    let ptr = ProjectPointer {
        project_id: res.project_id,
        project_code: res.project_code.clone(),
        project_name: res.project_name.clone(),
    };
    let cwd = std::env::current_dir()?;
    // 覆盖不同项目的既有指向时明示切换，避免无感知改写
    if let Some(old) = config::find_project_pointer(&cwd)?
        && old.project_id != res.project_id
    {
        out.kv(
            "替换指向",
            &format!(
                "{} (id={}) → {} (id={})",
                old.project_name, old.project_id, res.project_name, res.project_id
            ),
        );
    }
    let marker = config::save_project_pointer(&cwd, &ptr)?;

    let code_suffix = if res.project_code.is_empty() {
        String::new()
    } else {
        format!(", code={}", res.project_code)
    };
    out.kv(
        "project",
        &format!("{} (id={}{code_suffix})", res.project_name, res.project_id),
    );
    out.kv("写入", &marker.display().to_string());
    out.line("（下一步：bcode start 建立会话并查看开工包）");

    out.emit_value(&json!({
        "project": { "id": ptr.project_id, "code": ptr.project_code, "name": ptr.project_name },
        "pointer_path": marker.display().to_string(),
    }));
    Ok(())
}

/// `bcode start [--project <id|名>]`（F04）：建立/续期会话并展示开工包。
pub async fn start(
    cfg: &Config,
    profile: &str,
    project_flag: Option<&str>,
    out: &Out,
) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;

    // 项目解析：--project（code/数字 id/名称）优先，缺省取 .bc/project 指向
    let mut project = match project_flag {
        Some(spec) => {
            let client = BcodeClient::new(server.clone(), credential.clone(), None, cfg.insecure)?;
            let id = resolve_project(&client, spec).await?;
            cred::ProjectPointer {
                project_id: id,
                // --project 直连场景无 code，ensure_session 自动退回 id
                project_code: spec
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
                    .then(|| spec.to_string())
                    .filter(|s| s.parse::<i64>().is_err())
                    .unwrap_or_default(),
                project_name: String::new(),
            }
        }
        None => {
            let cwd = std::env::current_dir()?;
            config::find_project_pointer(&cwd)?.ok_or_else(|| {
                anyhow!("当前目录不在任何项目内：先 bcode join，或用 --project <code|id|名> 指定")
            })?
        }
    };
    // 服务端返回以 code 为准（跨环境稳定），回填指针
    let boot = ensure_session(&server, profile, &credential, &project, cfg.insecure).await?;
    if !boot.project.code.is_empty() {
        project.project_code = boot.project.code.clone();
        project.project_name = boot.project.name.clone();
        project.project_id = boot.project.id;
        let cwd = std::env::current_dir()?;
        let _ = config::save_project_pointer(&cwd, &project);
    }
    display_kickoff(&boot, out);
    out.emit_value(&serde_json::to_value(&boot)?);
    Ok(())
}

/// `bcode context`（F14）：开工包约定（conventions）原样输出——
/// AI 建立项目认知的入口。会话同键复用续期，此处重取保证约定最新。
pub async fn context(cfg: &Config, profile: &str, out: &Out) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    let cwd = std::env::current_dir()?;
    let ptr = config::find_project_pointer(&cwd)?.ok_or_else(|| {
        anyhow!("当前目录不在任何项目内：在项目仓库根目录执行，或先 bcode join <接入码>")
    })?;

    let boot = ensure_session(&server, profile, &credential, &ptr, cfg.insecure).await?;
    if boot.conventions.is_empty() {
        out.line("（无约定——项目与全局记忆均未配置 conventions.*）");
    }
    for c in &boot.conventions {
        out.line(&format!("[{}] {} = {}", c.scope, c.key, c.value));
    }
    out.emit_value(&json!({ "conventions": serde_json::to_value(&boot.conventions)? }));
    Ok(())
}

/// `bcode status`：打一次网络校验身份与会话的真实有效性（whoami 的在线版）。
/// 无凭证/无项目指向时给出分步引导而不是报错堆栈。
pub async fn status(cfg: &Config, profile: &str, out: &Out) -> Result<()> {
    let server = config::effective_server_url(cfg)?;
    let credential = cred::load_credential(profile)?;
    let agent_name = credential.name.clone();

    let cwd = std::env::current_dir()?;
    let Some(ptr) = config::find_project_pointer(&cwd)? else {
        bail!("当前目录不在任何项目内：在项目仓库根目录执行，或先 bcode join <接入码>");
    };

    let mut payload = json!({
        "agent": agent_name,
        "project": { "id": ptr.project_id, "name": ptr.project_name },
    });

    out.kv("agent", &agent_name);
    out.kv(
        "project",
        &format!("{} (id={})", ptr.project_name, ptr.project_id),
    );

    // 会话存在才带 X-Session 调免参任务端点（一举校验 key+准入+会话三件）；
    // 未建立会话是正常态，降级提示而非报错
    let Some(sess) = cred::load_session(profile, ptr.project_id) else {
        out.kv("连接", "凭证有效，但未建立会话（bcode start）");
        payload["connected"] = json!(false);
        payload["reason"] = json!("no_session");
        out.emit_value(&payload);
        return Ok(());
    };

    let client = BcodeClient::new(server, credential, Some(sess.session_id), cfg.insecure)?;
    let tasks = client.get_as::<AgentTasks>("/v1/agent/tasks").await?;

    out.kv("连接", "正常（身份/准入/会话全部有效）");
    out.kv("可见任务", &format!("{} 条", tasks.total));
    payload["connected"] = json!(true);
    payload["tasks_total"] = json!(tasks.total);

    out.emit_value(&payload);
    Ok(())
}

/// --project 解析：数字直接当 id；名称在已加入项目里匹配
/// （agent 视角 GET /v1/projects 只含已获准入的项目）
async fn resolve_project(client: &BcodeClient, spec: &str) -> Result<i64> {
    if let Ok(id) = spec.parse::<i64>() {
        return Ok(id);
    }
    // 名称解析：按 total 翻页，避免 >100 个已加入项目时漏配
    let mut page = 1;
    loop {
        let list: ProjectList = client
            .get_as(&format!("/v1/projects?page={page}&size=100"))
            .await?;
        if let Some(hit) = list.list.iter().find(|p| p.name == spec) {
            return Ok(hit.id);
        }
        if list.list.is_empty() || (page * 100) >= list.total {
            break;
        }
        page += 1;
    }
    Err(anyhow!(
        "未找到名为「{spec}」的项目（agent 只能看到已加入的项目；也可直接用数字 id）"
    ))
}

/// 开工包分段展示：约定 / 高频 QA / 我的任务 / 待审 / 待处理反馈 / 进行中专题
fn display_kickoff(boot: &SessionCreated, out: &Out) {
    out.kv("session", &boot.session_id);
    let code_suffix = if boot.project.code.is_empty() {
        String::new()
    } else {
        format!(", code={}", boot.project.code)
    };
    out.kv(
        "project",
        &format!(
            "{} (id={}{code_suffix})",
            boot.project.name, boot.project.id
        ),
    );
    out.line("");
    out.line(&format!("── 约定（{} 条）──", boot.conventions.len()));
    if boot.conventions.is_empty() {
        out.line("  （无约定——owner 可在 Web 维护项目记忆 conventions.*，agent 将自动消费）");
    }
    for c in &boot.conventions {
        out.line(&format!("  [{}] {} = {}", c.scope, c.key, c.value));
    }
    out.line("");
    out.line(&format!("── 我的任务（{}）──", boot.my_tasks.len()));
    if boot.my_tasks.is_empty() {
        out.line("  （无）");
    }
    for t in &boot.my_tasks {
        out.line(&format!("  {}", t.one_line()));
    }
    out.line("");
    out.line(&format!("── 待审（{}）──", boot.pending_reviews.len()));
    if boot.pending_reviews.is_empty() {
        out.line("  （无）");
    }
    for t in &boot.pending_reviews {
        out.line(&format!("  {}", t.one_line()));
    }
    out.line("");
    out.line(&format!(
        "── 高频 QA（{}）── 遇到问题先查 QA 库再问人",
        boot.top_qas.len()
    ));
    if boot.top_qas.is_empty() {
        out.line("  （无——bcode qa <关键词> 可搜索；踩坑后 qa add 沉淀）");
    }
    for q in &boot.top_qas {
        out.line(&format!("  #{} [{}] {}", q.id, q.hits, q.question));
    }
    out.line("");
    out.line(&format!(
        "── 待处理反馈（{}）── 阅读后 convert 建任务或 dismiss",
        boot.pending_feedbacks.len()
    ));
    if boot.pending_feedbacks.is_empty() {
        out.line("  （无）");
    }
    for f in &boot.pending_feedbacks {
        out.line(&format!(
            "  #{} [来自 {}] {}（bcode feedback convert {} / dismiss）",
            f.id, f.source_project_name, f.title, f.id
        ));
    }
    out.line("");
    out.line(&format!(
        "── 进行中专题（{}）── bcode topic work 推进",
        boot.active_topics.len()
    ));
    if boot.active_topics.is_empty() {
        out.line("  （无）");
    }
    for t in &boot.active_topics {
        out.line(&format!(
            "  #{} {}/{} 阶段 {} —— {}（bcode topic {}）",
            t.id, t.phase_done, t.phase_total, t.title, t.goal, t.id
        ));
    }
}
