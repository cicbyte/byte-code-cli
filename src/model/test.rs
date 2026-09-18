//! 测试执行记录域线缆类型（源：byte-code `api/v1/test`，#505）。
//! 上报体 TestRunReport 同时是 byte-code-pytest `--bcode-dump` 离线文件的载荷
//! （外面包一层 TestRunDumpFile 做格式自识别）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// POST /v1/projects/{id}/test-runs 请求体
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunReport {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub git_sha: String,
    #[serde(default)]
    pub env: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub finished_at: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub cases: Vec<TestRunCaseReport>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunCaseReport {
    #[serde(default)]
    pub test_case_id: i64,
    #[serde(default)]
    pub external_key: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub message: String,
}

/// byte-code-pytest --bcode-dump 离线文件壳：format 标记用于与 junit XML 自动区分
#[derive(Debug, Serialize, Deserialize)]
pub struct TestRunDumpFile {
    pub format: String,
    #[serde(default)]
    pub version: i64,
    pub payload: TestRunReport,
}

/// GET /v1/projects/{id}/test-cases 响应
#[derive(Debug, Deserialize)]
pub struct TestCaseList {
    #[serde(default)]
    pub total: i64,
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<TestCaseItem>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TestCaseItem {
    pub id: i64,
    pub project_id: i64,
    pub requirement_id: i64,
    pub task_id: i64,
    pub title: String,
    pub preconditions: String,
    pub steps: String,
    pub expected_result: String,
    pub category: String,
    pub module: String,
    pub priority: String,
    pub source: String,
    pub status: String,
    pub external_key: String,
}
