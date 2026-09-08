//! QA 库端点类型（源：`api/v1/project/qa.go`）。

use serde::{Deserialize, Serialize};

use super::null_to_default;

/// GET /v1/projects/{id}/qas 响应（按 hits 降序）
#[derive(Debug, Serialize, Deserialize)]
pub struct QaList {
    #[serde(default, deserialize_with = "null_to_default")]
    pub list: Vec<QaItem>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaItem {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub question: String,
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub hits: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub updater: String,
    #[serde(default)]
    pub updated_at: String,
}

/// POST /v1/projects/{id}/qas 响应（按问题去重 upsert）
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QaUpsertResult {
    #[serde(default)]
    pub id: i64,
    /// true=更新了已有条目
    #[serde(default)]
    pub updated: bool,
}
