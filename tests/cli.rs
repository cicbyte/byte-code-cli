//! CLI 对外契约测试：退出码表（error.rs）与 --json 输出形状。
//! 通过 BC_HOME + 临时 cwd 隔离，全程不触碰真实数据目录与项目外目录。

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

/// 每个用例独立的沙箱：临时目录同时充当 BC_HOME 与 cwd
struct Sandbox {
    #[allow(dead_code)]
    _dir: TempDir,
    root: std::path::PathBuf,
}

impl Sandbox {
    fn new() -> (Self, Command) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let mut cmd = Command::cargo_bin("bcode").unwrap();
        cmd.env("BC_HOME", &root)
            .env_remove("BC_AGENT")
            .current_dir(&root);
        (Self { _dir: dir, root }, cmd)
    }

    fn write_config(&self, body: &str) {
        std::fs::write(self.root.join("config.toml"), body).unwrap();
    }

    fn write_credential(&self, profile: &str, name: &str, agent_id: i64) {
        let dir = self.root.join("agents").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        let cred = serde_json::json!({ "name": name, "agent_id": agent_id, "api_key": "bc_test" });
        std::fs::write(dir.join("credential"), cred.to_string()).unwrap();
    }

    fn write_project_pointer(&self, project_id: i64, project_name: &str) {
        std::fs::create_dir_all(self.root.join(".bc")).unwrap();
        let ptr = serde_json::json!({ "project_id": project_id, "project_name": project_name });
        std::fs::write(self.root.join(".bc").join("project"), ptr.to_string()).unwrap();
    }

    fn write_session(&self, profile: &str, project_id: i64, session_id: &str) {
        let dir = self.root.join("sessions").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        let s = serde_json::json!({
            "session_id": session_id, "project_id": project_id, "project_name": "demo"
        });
        std::fs::write(dir.join(format!("{project_id}.json")), s.to_string()).unwrap();
    }
}

fn json_stdout(mut cmd: Command) -> Value {
    let out = cmd.unwrap();
    serde_json::from_slice(&out.stdout).expect("stdout 应为合法 JSON")
}

#[test]
fn whoami_reports_empty_state_with_exit_zero() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.args(["whoami", "--json"]);
    let v = json_stdout(cmd);
    assert_eq!(v["profile"], "default");
    assert_eq!(v["has_credential"], false);
    assert_eq!(v["server"], Value::Null);
    assert_eq!(v["project"], Value::Null);
}

#[test]
fn whoami_aggregates_local_state() {
    let (sb, mut cmd) = Sandbox::new();
    sb.write_config("server_url = \"http://127.0.0.1:8000/api\"\ndefault_profile = \"alice\"");
    sb.write_credential("alice", "alice-cli", 42);
    sb.write_project_pointer(7, "demo");
    sb.write_session("alice", 7, "sess-1234567890");
    cmd.args(["whoami", "--json"]);
    let v = json_stdout(cmd);
    assert_eq!(v["profile"], "alice");
    assert_eq!(v["agent"], "alice-cli");
    assert_eq!(v["agent_id"], 42);
    assert_eq!(v["has_credential"], true);
    assert_eq!(v["server"], "http://127.0.0.1:8000/api");
    assert_eq!(v["project"]["id"], 7);
    assert_eq!(v["project"]["name"], "demo");
    assert_eq!(v["session"], "sess-1234567890");
}

#[test]
fn whoami_human_mode_prints_kv_lines() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.args(["whoami"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("profile: default"));
    assert!(!text.starts_with('{'));
}

#[test]
fn profiles_lists_local_identities() {
    let (sb, mut cmd) = Sandbox::new();
    sb.write_credential("alice", "alice-cli", 1);
    sb.write_credential("bob", "bob-cli", 2);
    cmd.args(["profiles", "--json"]);
    let v = json_stdout(cmd);
    let arr = v["profiles"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["profile"], "alice");
    assert_eq!(arr[0]["agent"], "alice-cli");
    assert_eq!(v["current"], "default");
}

#[test]
fn profile_flag_overrides_bc_agent_env() {
    let (sb, mut cmd) = Sandbox::new();
    sb.write_credential("from-flag", "f", 1);
    cmd.env("BC_AGENT", "from-env")
        .args(["--profile", "from-flag", "whoami", "--json"]);
    let v = json_stdout(cmd);
    assert_eq!(v["profile"], "from-flag");
}

#[test]
fn bc_agent_env_selects_profile() {
    let (sb, mut cmd) = Sandbox::new();
    sb.write_credential("from-env", "e", 1);
    cmd.env("BC_AGENT", "from-env").args(["whoami", "--json"]);
    let v = json_stdout(cmd);
    assert_eq!(v["profile"], "from-env");
    assert_eq!(v["has_credential"], true);
}

#[test]
fn status_exits_2_without_server_url() {
    let (sb, mut cmd) = Sandbox::new();
    sb.write_credential("alice", "alice-cli", 1);
    cmd.args(["status"]);
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("未配置平台地址"));
}

#[test]
fn status_exits_2_outside_any_project() {
    let (sb, mut cmd) = Sandbox::new();
    sb.write_config("server_url = \"http://127.0.0.1:8000/api\"\ndefault_profile = \"alice\"");
    sb.write_credential("alice", "alice-cli", 1);
    cmd.args(["status"]);
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("不在任何项目"));
}

#[test]
fn help_lists_full_command_surface() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("register"))
        .stdout(predicate::str::contains("claim"))
        .stdout(predicate::str::contains("notify"))
        .stdout(predicate::str::contains("search"));
}

#[test]
fn init_writes_config_in_noninteractive_mode() {
    let (sb, mut cmd) = Sandbox::new();
    let _ = sb;
    // 指向不可达端口：探测失败仅警告，配置仍写入（离线可测）
    cmd.args(["init", "http://127.0.0.1:9/api"]);
    cmd.assert().success();
    let raw = std::fs::read_to_string(sb.root.join("config.toml")).unwrap();
    assert!(
        raw.contains(r#"server_url = "http://127.0.0.1:9/api""#),
        "config 未写入：{raw}"
    );
}

#[test]
fn init_rejects_non_http_url() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.args(["init", "127.0.0.1:8000"]);
    cmd.assert().failure().code(2);
}

#[test]
fn completion_and_man_smoke() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.args(["completion", "bash"]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let script = String::from_utf8(out).unwrap();
    assert!(script.contains("register"), "补全脚本缺少子命令");

    let (_sb2, mut cmd2) = Sandbox::new();
    cmd2.arg("man");
    let out2 = cmd2.assert().success().get_output().stdout.clone();
    assert!(String::from_utf8(out2).unwrap().contains("bcode"));
}

#[test]
fn profile_name_with_path_separator_is_rejected() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.args(["--profile", "../evil", "whoami"]);
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("profile 名不合法"));
}

#[test]
fn complete_rejects_oversized_artifacts_before_network() {
    let (sb, mut cmd) = Sandbox::new();
    let big = sb.root.join("big.md");
    std::fs::write(&big, "x".repeat(2 * 1024 * 1024)).unwrap();
    cmd.args(["complete", "1", "--artifacts-file", big.to_str().unwrap()]);
    // 上限校验先于联网：沙箱无 server 配置也应报「过大」而非「未配置平台地址」
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("上限 1 MiB"));
}

#[test]
fn create_requires_title_before_any_network() {
    let (_sb, mut cmd) = Sandbox::new();
    cmd.args(["create"]); // 无 --title 无 --file：应在联网前报错
    cmd.assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("title 必填"));
}

#[test]
fn init_auto_appends_api_prefix() {
    let (sb, mut cmd) = Sandbox::new();
    cmd.args(["init", "http://127.0.0.1:9"]); // 无路径：自动补 /api
    cmd.assert().success();
    let raw = std::fs::read_to_string(sb.root.join("config.toml")).unwrap();
    assert!(
        raw.contains(r#"server_url = "http://127.0.0.1:9/api""#),
        "未自动补 /api：{raw}"
    );
}
