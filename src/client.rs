//! 平台 HTTP 客户端：认证头注入（Bearer bc_xxx）+ X-Session（有会话时）
//! + 统一响应壳解包。网络层错误归类 BcodeError::Network。

use anyhow::Result;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::cred::Credential;
use crate::error::BcodeError;
use crate::model::ApiEnvelope;

pub struct BcodeClient {
    http: Client,
    server: String,
    credential: Credential,
    session: Option<String>,
}

impl BcodeClient {
    pub fn new(server: String, credential: Credential, session: Option<String>) -> Result<Self> {
        let http = Client::builder()
            .user_agent(concat!("bcode/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| BcodeError::Network(e.to_string()))?;
        Ok(Self {
            http,
            server,
            credential,
            session,
        })
    }

    // M1：start 建立会话后注入
    #[allow(dead_code)]
    pub fn with_session(mut self, sid: impl Into<String>) -> Self {
        self.session = Some(sid.into());
        self
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.request(method, &url).header(
            "Authorization",
            format!("Bearer {}", self.credential.api_key),
        );
        if let Some(sid) = &self.session {
            req = req.header("X-Session", sid);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| BcodeError::Network(e.to_string()))?;
        let status = resp.status();
        // 平台错误可能带 200+壳码 或 401/403 HTTP 码，两条路都收敛到壳
        let envelope: ApiEnvelope = resp
            .json()
            .await
            .map_err(|e| BcodeError::Network(format!("响应解析失败（HTTP {status}）：{e}")))?;
        if status.as_u16() == 401 {
            return Err(BcodeError::Auth(envelope.message).into());
        }
        Ok(envelope.into_data()?)
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        self.request(reqwest::Method::GET, path, None).await
    }

    /// GET 并按 model 层强类型解码（解码失败按网络类错误归档）
    pub async fn get_as<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let v = self.get(path).await?;
        serde_json::from_value(v)
            .map_err(|e| BcodeError::Network(format!("响应结构不符：{e}")).into())
    }

    // M1：claim/complete/log 等写操作接入
    #[allow(dead_code)]
    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, path, Some(body)).await
    }
}
