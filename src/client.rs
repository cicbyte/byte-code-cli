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
    credential: Option<Credential>,
    session: Option<String>,
}

impl BcodeClient {
    pub fn new(
        server: String,
        credential: Credential,
        session: Option<String>,
        insecure: bool,
    ) -> Result<Self> {
        Self::build(server, Some(credential), session, insecure)
    }

    /// 无认证客户端（register 公开端点用）
    pub fn anonymous(server: String, insecure: bool) -> Result<Self> {
        Self::build(server, None, None, insecure)
    }

    fn build(
        server: String,
        credential: Option<Credential>,
        session: Option<String>,
        insecure: bool,
    ) -> Result<Self> {
        let mut builder = Client::builder()
            .user_agent(concat!("bcode/", env!("CARGO_PKG_VERSION")))
            // 连接级超时全体生效（含 SSE 长连接的建立阶段）；
            // 总超时不设在客户端上——SSE 需要无限期，常规请求按请求级覆盖
            .connect_timeout(std::time::Duration::from_secs(10));
        if insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let http = builder
            .build()
            .map_err(|e| BcodeError::Network(e.to_string()))?;
        Ok(Self {
            http,
            server,
            credential,
            session,
        })
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.request(method, &url);
        if let Some(c) = &self.credential {
            req = req.header("Authorization", format!("Bearer {}", c.api_key));
        }
        if let Some(sid) = &self.session {
            req = req.header("X-Session", sid);
        }
        if let Some(b) = body {
            req = req.json(&b);
        }
        // 常规请求总超时（连接+读整体）；SSE 流不走本方法，不受影响
        let resp = req
            .timeout(std::time::Duration::from_secs(30))
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
        if status.as_u16() == 403 {
            return Err(BcodeError::Forbidden(envelope.message).into());
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

    pub async fn post(&self, path: &str, body: Value) -> Result<Value> {
        self.request(reqwest::Method::POST, path, Some(body)).await
    }

    /// POST 并按 model 层强类型解码
    pub async fn post_as<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T> {
        let v = self.post(path, body).await?;
        serde_json::from_value(v)
            .map_err(|e| BcodeError::Network(format!("响应结构不符：{e}")).into())
    }

    /// 打开 SSE 长连接（notify --watch）：认证与状态检查在此，流由调用方消费
    pub async fn open_stream(&self, path: &str) -> Result<reqwest::Response> {
        let url = format!("{}{}", self.server, path);
        let mut req = self.http.get(&url);
        if let Some(c) = &self.credential {
            req = req.header("Authorization", format!("Bearer {}", c.api_key));
        }
        let resp = req
            .send()
            .await
            .map_err(|e| BcodeError::Network(e.to_string()))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(BcodeError::Network(format!("SSE 连接失败（HTTP {status}）")).into());
        }
        Ok(resp)
    }
}

/// 最小百分号编码（query 值用）：非保留字符外的字节转 %XX
pub fn encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_query_keeps_unreserved_and_encodes_rest() {
        assert_eq!(encode_query("AZaz09-_.~"), "AZaz09-_.~");
        assert_eq!(encode_query("a b&c=1"), "a%20b%26c%3D1");
        assert_eq!(encode_query("约定"), "%E7%BA%A6%E5%AE%9A");
    }
}
