//! 测试执行域命令：`bcode test --run -- <cmd>` 包裹执行、
//! `--upload <file>` 离线补传（junit XML / byte-code-pytest dump JSON 自动识别）、
//! `--cases --pull|--push` 平台用例与仓库 tests/ 目录 YAML 双向同步。

use anyhow::{Result, anyhow};
use serde_json::json;

use super::project_ctx;
use crate::cli::TestArgs;
use crate::client::encode_query;
use crate::config::Config;
use crate::model::task::CreatedId;
use crate::model::test::{TestCaseItem, TestCaseList, TestRunDumpFile, TestRunReport};
use crate::output::Out;

/// `bcode test --run -- <cmd...> [--junit <path>]`：执行测试命令，
/// 产出 junit 文件则顺手上报；退出码透传被包裹命令
/// `bcode test --upload <file> [--format junit|bcode] [--source] [--branch]`
/// `bcode test --cases --pull [--dir]` / `--cases --push [--dir] [--dry-run]`
pub async fn test(cfg: &Config, profile: &str, a: &TestArgs, out: &Out) -> Result<()> {
    if a.cases {
        if a.pull == a.push {
            return Err(anyhow!(
                "--cases 需要二选一：--pull（拉取到目录）或 --push（推送回平台）"
            ));
        }
        let ctx = project_ctx(cfg, profile).await?;
        let pid = ctx.project.project_id;
        if a.pull {
            cases_pull(&ctx.client, pid, &a.dir, a.status.as_deref(), out).await
        } else {
            cases_push(&ctx.client, pid, &a.dir, a.dry_run, out).await
        }
    } else if let Some(file) = a.upload.as_deref() {
        let ctx = project_ctx(cfg, profile).await?;
        let pid = ctx.project.project_id;
        let mut report = load_report(file, a.format.as_deref())?;
        if let Some(s) = a.source.as_deref() {
            report.source = s.to_string();
        }
        if let Some(b) = a.branch.as_deref() {
            report.branch = b.to_string();
        }
        if let Some(e) = a.env.as_deref() {
            report.env = e.to_string();
        }
        if report.branch.is_empty() {
            report.branch = git_out(&["rev-parse", "--abbrev-ref", "HEAD"]);
        }
        if report.git_sha.is_empty() {
            report.git_sha = git_out(&["rev-parse", "HEAD"]);
        }
        let created: CreatedId = ctx
            .client
            .post_as(
                &format!("/v1/projects/{pid}/test-runs"),
                serde_json::to_value(&report)?,
            )
            .await?;
        out.kv(
            "已上报",
            &format!(
                "run #{}（{} 用例，项目 {pid}）",
                created.id,
                report.cases.len()
            ),
        );
        out.emit_value(&json!({ "run_id": created.id, "cases": report.cases.len() }));
        Ok(())
    } else if a.run {
        run_wrapped(cfg, profile, a, out).await
    } else {
        Err(anyhow!(
            "缺子动作：--run -- <命令> 执行并上报 / --upload <文件> 离线补传 / --cases --pull|--push 用例同步"
        ))
    }
}

/// 包裹执行：stdout/stderr 直通（保留实时输出与颜色），退出码透传；
/// 带 --junit 时命令结束后把该文件按 junit 上报
async fn run_wrapped(cfg: &Config, profile: &str, a: &TestArgs, out: &Out) -> Result<()> {
    if a.cmd.is_empty() {
        return Err(anyhow!(
            "--run 需要 -- 后接完整命令，如：bcode test --run -- pytest -q --junitxml=.bc/junit.xml"
        ));
    }
    let (prog, args) = a.cmd.split_first().unwrap();
    let joined = a.cmd.join(" ");
    out.kv("执行", &joined);

    let status = std::process::Command::new(prog)
        .args(args)
        .status()
        .map_err(|e| anyhow!("启动 {prog} 失败：{e}（确认命令存在）"))?;

    let code = status.code().unwrap_or(1);
    if let Some(junit) = a.junit.as_deref() {
        match upload_after_run(cfg, profile, a, junit).await {
            Ok(run_id) => out.kv("已上报", &format!("run #{run_id}（junit: {junit}）")),
            Err(e) => {
                // 上报失败不吞测试结论：警示但按被包裹命令的退出码收口
                out.line(&format!("  上报失败：{e}"));
            }
        }
    }
    std::process::exit(code);
}

/// run 模式的 junit 上报（独立小函数便于错误隔离）
async fn upload_after_run(cfg: &Config, profile: &str, a: &TestArgs, junit: &str) -> Result<i64> {
    let ctx = project_ctx(cfg, profile).await?;
    let mut report = load_report(junit, Some("junit"))?;
    if let Some(s) = a.source.as_deref() {
        report.source = s.to_string();
    }
    if report.env.is_empty() {
        report.env = "local".into();
    }
    if report.branch.is_empty() {
        report.branch = git_out(&["rev-parse", "--abbrev-ref", "HEAD"]);
    }
    if report.git_sha.is_empty() {
        report.git_sha = git_out(&["rev-parse", "HEAD"]);
    }
    let created: CreatedId = ctx
        .client
        .post_as(
            &format!("/v1/projects/{}/test-runs", ctx.project.project_id),
            serde_json::to_value(&report)?,
        )
        .await?;
    Ok(created.id)
}

/// 读取上报文件并解析为统一载荷；format=auto 时按内容嗅探（XML → junit，
/// JSON 带 format=bcode-test-run 壳 → 插件 dump）
pub fn load_report(path: &str, format: Option<&str>) -> Result<TestRunReport> {
    let raw = std::fs::read_to_string(path).map_err(|e| anyhow!("读取 {path} 失败：{e}"))?;
    let fmt = format.unwrap_or("auto");
    let is_xml = raw.trim_start().starts_with('<');
    match fmt {
        "junit" => Ok(parse_junit(&raw)?),
        "bcode" => parse_dump(&raw),
        _ => {
            if is_xml {
                parse_junit(&raw)
            } else {
                parse_dump(&raw)
            }
        }
    }
}

fn parse_dump(raw: &str) -> Result<TestRunReport> {
    let f: TestRunDumpFile = serde_json::from_str(raw)
        .map_err(|e| anyhow!("不是有效的 bcode dump JSON（应来自 pytest --bcode-dump）：{e}"))?;
    if f.format != "bcode-test-run" {
        return Err(anyhow!("JSON 缺格式标记：期望 format=bcode-test-run"));
    }
    Ok(f.payload)
}

/// junit XML（pytest-junit / vitest / go test -v junit 输出同构）→ 上报载荷。
/// external_key = classname::name（classname 缺省用 suite name）；汇总计数由
/// 服务端重算，这里只逐用例转换
pub fn parse_junit(xml: &str) -> Result<TestRunReport> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| anyhow!("junit XML 解析失败：{e}"))?;
    let root = doc.root_element();
    // 兼容两种根：<testsuites> 包裹 与 单一 <testsuite>
    let suites: Vec<roxmltree::Node> = if root.has_tag_name("testsuites") {
        root.children()
            .filter(|n| n.has_tag_name("testsuite"))
            .collect()
    } else if root.has_tag_name("testsuite") {
        vec![root]
    } else {
        return Err(anyhow!("不是 junit XML：根元素应为 testsuites/testsuite"));
    };

    let mut cases = Vec::new();
    for suite in &suites {
        let suite_name = suite.attribute("name").unwrap_or("");
        for tc in suite.children().filter(|n| n.has_tag_name("testcase")) {
            let classname = tc.attribute("classname").unwrap_or(suite_name);
            let name = tc.attribute("name").unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let external_key = if classname.is_empty() {
                name.to_string()
            } else {
                format!("{classname}::{name}")
            };
            // 子元素定状态：failure=fail / error=error / skipped=skip，无则 pass
            let mut status = "pass";
            let mut message = String::new();
            for child in tc.children().filter(|n| n.is_element()) {
                let tag = child.tag_name().name();
                match tag {
                    "failure" | "error" => {
                        status = if tag == "failure" { "fail" } else { "error" };
                        message = child_text(child);
                    }
                    "skipped" => status = "skip",
                    "system-out" | "system-err" => {}
                    _ => {}
                }
            }
            let duration_ms = (tc
                .attribute("time")
                .and_then(|t| t.parse::<f64>().ok())
                .unwrap_or(0.0)
                * 1000.0)
                .round() as i64;
            cases.push(crate::model::test::TestRunCaseReport {
                test_case_id: 0,
                title: name.to_string(),
                external_key,
                status: status.to_string(),
                duration_ms,
                message: truncate_message(&message),
            });
        }
    }
    if cases.is_empty() {
        return Err(anyhow!("junit XML 里没有 testcase（文件损坏或格式不符）"));
    }
    let started = suites
        .iter()
        .filter_map(|s| s.attribute("timestamp"))
        .min()
        .map(normalize_timestamp)
        .unwrap_or_default();
    let total_ms = suites
        .iter()
        .filter_map(|s| s.attribute("time").and_then(|t| t.parse::<f64>().ok()))
        .sum::<f64>();
    Ok(TestRunReport {
        idempotency_key: String::new(),
        source: "junit".into(),
        branch: String::new(),
        git_sha: String::new(),
        env: String::new(),
        started_at: started,
        finished_at: String::new(),
        duration_ms: (total_ms * 1000.0).round() as i64,
        cases,
    })
}

/// junit timestamp 是 ISO8601（含 T 与时区），平台列约定 YYYY-MM-DD HH:MM:SS
/// 本地时间——取前 19 位并把 T 换空格（丢弃亚秒/时区，仅作展示）
fn normalize_timestamp(ts: &str) -> String {
    let head: String = ts.chars().take(19).collect();
    head.replace('T', " ")
}

fn child_text(n: roxmltree::Node) -> String {
    n.text().unwrap_or("").to_string()
}

fn truncate_message(s: &str) -> String {
    const LIMIT: usize = 4000;
    if s.chars().count() <= LIMIT {
        return s.to_string();
    }
    let cut: String = s.chars().take(LIMIT).collect();
    cut + "\n...[truncated by bcode-cli]"
}

/// git 元数据探测（非 git 目录静默空串）
pub fn git_out(args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

// ==================== 用例双向同步 ====================

/// pull：平台用例分页拉全，逐条写 YAML（external_key 命名优先，跨环境稳定）
async fn cases_pull(
    client: &crate::client::BcodeClient,
    pid: i64,
    dir: &str,
    status: Option<&str>,
    out: &Out,
) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| anyhow!("创建 {dir} 失败：{e}"))?;
    let status = status.unwrap_or("active");
    let mut page = 1;
    let mut total = -1i64;
    let mut written = 0usize;
    loop {
        let list: TestCaseList = client
            .get_as(&format!(
                "/v1/projects/{pid}/test-cases?pageNum={page}&pageSize=100&status={}",
                encode_query(status)
            ))
            .await?;
        if total < 0 {
            total = list.total;
            if total == 0 {
                out.line(&format!("（平台无 {status} 用例，目录未变更）"));
                return Ok(());
            }
        }
        for c in &list.list {
            let path = format!("{}/{}.yaml", dir, case_file_stem(c));
            let yaml =
                serde_yaml::to_string(c).map_err(|e| anyhow!("序列化用例 #{} 失败：{e}", c.id))?;
            std::fs::write(&path, yaml).map_err(|e| anyhow!("写 {path} 失败：{e}"))?;
            written += 1;
        }
        if (page as i64) * 100 >= total {
            break;
        }
        page += 1;
    }
    out.kv(
        "已拉取",
        &format!("{written} 条 → {dir}/（status={status}）"),
    );
    out.emit_value(&json!({ "pulled": written, "dir": dir }));
    Ok(())
}

/// 文件名主干：external_key 清洗（nodeid 的 :: 与路径分隔符统一成 .），
/// 缺省用 case-{id}；超长截断防 Windows 路径上限
fn case_file_stem(c: &TestCaseItem) -> String {
    let raw = if !c.external_key.is_empty() {
        c.external_key.clone()
    } else {
        format!("case-{}", c.id)
    };
    let cleaned: String = raw
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => ch,
            _ => '.',
        })
        // nodeid 的 :: 与路径分隔符映射成点后折叠连续点
        .fold(String::new(), |mut acc, ch| {
            if ch == '.' && acc.ends_with('.') {
                return acc;
            }
            acc.push(ch);
            acc
        });
    let trimmed = cleaned.trim_matches('.').to_string();
    let stem = if trimmed.is_empty() {
        format!("case-{}", c.id)
    } else {
        trimmed
    };
    stem.chars().take(80).collect()
}

/// push：逐 YAML upsert——id>0 直接更新；id=0 且有 externalKey 先查再建；
/// 都没有则新建。--dry-run 只打印计划
async fn cases_push(
    client: &crate::client::BcodeClient,
    pid: i64,
    dir: &str,
    dry_run: bool,
    out: &Out,
) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|_| anyhow!("目录 {dir} 不存在：先 --cases --pull 生成，或手工创建"))?;
    let mut files: Vec<String> = entries
        .flatten()
        .map(|e| e.path().to_string_lossy().to_string())
        .filter(|p| p.ends_with(".yaml") || p.ends_with(".yml"))
        .collect();
    files.sort();
    if files.is_empty() {
        out.line(&format!("（{dir}/ 下没有 YAML 用例文件）"));
        return Ok(());
    }

    let mut created = 0usize;
    let mut updated = 0usize;
    for file in &files {
        let raw = std::fs::read_to_string(file).map_err(|e| anyhow!("读取 {file} 失败：{e}"))?;
        let mut c: TestCaseItem =
            serde_yaml::from_str(&raw).map_err(|e| anyhow!("{file} 不是合法用例 YAML：{e}"))?;
        if c.title.trim().is_empty() {
            return Err(anyhow!("{file} 缺 title（平台必填）"));
        }
        if c.priority.is_empty() {
            c.priority = "P2".into();
        }
        if c.status.is_empty() {
            c.status = "active".into();
        }

        // 解析目标 id：显式 id > externalKey 查找 > 新建
        let mut target = c.id;
        if target <= 0 && !c.external_key.is_empty() {
            let found: TestCaseList = client
                .get_as(&format!(
                    "/v1/projects/{pid}/test-cases?externalKey={}&pageSize=1",
                    encode_query(&c.external_key)
                ))
                .await?;
            if let Some(hit) = found.list.first() {
                target = hit.id;
            }
        }

        if target > 0 {
            if dry_run {
                out.line(&format!("  更新 #{} ← {}（{}）", target, file, c.title));
                updated += 1;
                continue;
            }
            client
                .put(
                    &format!("/v1/test-cases/{target}"),
                    json!({
                        "title": c.title,
                        "preconditions": c.preconditions,
                        "steps": c.steps,
                        "expectedResult": c.expected_result,
                        "category": c.category,
                        "module": c.module,
                        "priority": c.priority,
                        "externalKey": c.external_key,
                        "status": c.status,
                        "requirementId": c.requirement_id,
                        "taskId": c.task_id,
                    }),
                )
                .await?;
            updated += 1;
        } else {
            if dry_run {
                out.line(&format!("  新建　  ← {}（{}）", file, c.title));
                created += 1;
                continue;
            }
            let made: CreatedId = client
                .post_as(
                    &format!("/v1/projects/{pid}/test-cases"),
                    json!({
                        "title": c.title,
                        "preconditions": c.preconditions,
                        "steps": c.steps,
                        "expectedResult": c.expected_result,
                        "category": c.category,
                        "module": c.module,
                        "priority": c.priority,
                        "externalKey": c.external_key,
                        "requirementId": c.requirement_id,
                        "taskId": c.task_id,
                    }),
                )
                .await?;
            out.line(&format!("  已新建 #{}（{}）", made.id, c.title));
            created += 1;
        }
    }
    out.kv(
        "同步完成",
        &format!("新建 {created} / 更新 {updated}（目录 {dir}）"),
    );
    out.emit_value(&json!({ "created": created, "updated": updated, "dir": dir }));
    Ok(())
}

// ==================== 单元测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    const JUNIT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="pytest" tests="4" failures="1" errors="1" skipped="1" time="0.05">
  <testsuite name="pytest" tests="4" failures="1" errors="1" skipped="1" time="0.05" timestamp="2026-09-18T20:15:30.123456+08:00">
    <testcase classname="tests.test_login" name="test_ok" time="0.01"/>
    <testcase classname="tests.test_login" name="test_bad" time="0.002">
      <failure message="assert 1 == 2">def test_bad():
&gt;       assert 1 == 2
E       assert 1 == 2</failure>
    </testcase>
    <testcase classname="tests.test_api" name="test_boom" time="0.001">
      <error message="fixture boom">RuntimeError: fixture boom</error>
    </testcase>
    <testcase classname="tests.test_api" name="test_later" time="0.001">
      <skipped type="pytest.skip" message="later">skipped</skipped>
    </testcase>
  </testsuite>
</testsuites>"#;

    #[test]
    fn junit_parse_maps_statuses_and_keys() {
        let r = parse_junit(JUNIT).unwrap();
        assert_eq!(r.source, "junit");
        assert_eq!(r.started_at, "2026-09-18 20:15:30");
        assert_eq!(r.duration_ms, 50);
        assert_eq!(r.cases.len(), 4);
        let by: std::collections::HashMap<&str, &crate::model::test::TestRunCaseReport> = r
            .cases
            .iter()
            .map(|c| (c.external_key.as_str(), c))
            .collect();
        let ok = by["tests.test_login::test_ok"];
        assert_eq!(
            (ok.status.as_str(), ok.test_case_id, ok.duration_ms),
            ("pass", 0, 10)
        );
        let bad = by["tests.test_login::test_bad"];
        assert_eq!(bad.status, "fail");
        assert!(bad.message.contains("assert 1 == 2"));
        let err = by["tests.test_api::test_boom"];
        assert_eq!(err.status, "error");
        assert!(err.message.contains("fixture boom"));
        let skip = by["tests.test_api::test_later"];
        assert_eq!(skip.status, "skip");
    }

    #[test]
    fn junit_rejects_non_junit_root() {
        assert!(parse_junit("<html><body>x</body></html>").is_err());
        assert!(parse_junit(r#"<testsuites></testsuites>"#).is_err()); // 无 testcase
    }

    #[test]
    fn dump_roundtrip_and_sniff() {
        let r = parse_junit(JUNIT).unwrap();
        let dump = TestRunDumpFile {
            format: "bcode-test-run".into(),
            version: 1,
            payload: r,
        };
        let raw = serde_json::to_string(&dump).unwrap();
        // 嗅探路径：JSON 内容 → dump 解析回同构载荷
        let back = load_report_from_str(&raw, None).unwrap();
        assert_eq!(back.cases.len(), 4);
        assert_eq!(back.source, "junit");
        // 壳标记缺失要拒
        assert!(load_report_from_str(r#"{"foo":1}"#, Some("bcode")).is_err());
    }

    /// load_report 的纯字符串变体（单测不落盘）
    fn load_report_from_str(raw: &str, format: Option<&str>) -> Result<TestRunReport> {
        let is_xml = raw.trim_start().starts_with('<');
        match format.unwrap_or("auto") {
            "junit" => parse_junit(raw),
            "bcode" => parse_dump(raw),
            _ => {
                if is_xml {
                    parse_junit(raw)
                } else {
                    parse_dump(raw)
                }
            }
        }
    }

    #[test]
    fn case_yaml_roundtrip() {
        let c = TestCaseItem {
            id: 7,
            title: "登录成功".into(),
            preconditions: "已注册账号".into(),
            steps: "1. 打开登录页\n2. 输入账密".into(),
            expected_result: "跳转首页".into(),
            category: "功能".into(),
            module: "auth".into(),
            priority: "P1".into(),
            external_key: "tests/test_login.py::test_ok".into(),
            ..Default::default()
        };
        let yaml = serde_yaml::to_string(&c).unwrap();
        let back: TestCaseItem = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id, 7);
        assert_eq!(back.title, "登录成功");
        assert_eq!(back.external_key, "tests/test_login.py::test_ok");
        assert_eq!(back.steps, "1. 打开登录页\n2. 输入账密");
    }

    #[test]
    fn file_stem_sanitizes_nodeid() {
        let c = TestCaseItem {
            id: 3,
            external_key: "tests/test_login.py::test_ok".into(),
            ..Default::default()
        };
        assert_eq!(case_file_stem(&c), "tests.test_login.py.test_ok");
        let nokey = TestCaseItem {
            id: 42,
            ..Default::default()
        };
        assert_eq!(case_file_stem(&nokey), "case-42");
    }

    #[test]
    fn message_truncated_at_limit() {
        let long = "x".repeat(5000);
        let t = truncate_message(&long);
        assert!(t.chars().count() <= 4030);
        assert!(t.ends_with("...[truncated by bcode-cli]"));
    }
}
