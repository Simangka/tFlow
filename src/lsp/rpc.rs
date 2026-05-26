use crate::lsp::types::{RequestId, next_request_id};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, AsyncReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

#[derive(Debug)]
pub enum RpcMessage {
    Request {
        id: RequestId,
        method: String,
        params: Option<serde_json::Value>,
    },
    Response {
        id: RequestId,
        result: Option<serde_json::Value>,
        error: Option<JsonRpcError>,
    },
    Notification {
        method: String,
        params: Option<serde_json::Value>,
    },
}

#[derive(Serialize, Deserialize)]
struct RawMessage {
    #[serde(default = "default_jsonrpc")]
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

fn default_jsonrpc() -> String {
    "2.0".to_string()
}

impl RawMessage {
    fn into_message(self) -> RpcMessage {
        match (self.id, self.method) {
            (Some(id), Some(method)) => {
                let rid = id.as_u64().unwrap_or(0);
                RpcMessage::Request { id: rid, method, params: self.params }
            }
            (Some(id), None) => {
                let rid = id.as_u64().unwrap_or(0);
                RpcMessage::Response { id: rid, result: self.result, error: self.error }
            }
            (None, Some(method)) => {
                RpcMessage::Notification { method, params: self.params }
            }
            (None, None) => {
                tracing::warn!("Invalid JSON-RPC message: no id or method");
                RpcMessage::Notification { method: String::new(), params: None }
            }
        }
    }
}

pub struct RpcWriter {
    inner: tokio::io::BufWriter<ChildStdin>,
}

impl RpcWriter {
    pub fn new(stdin: ChildStdin) -> Self {
        Self {
            inner: tokio::io::BufWriter::new(stdin),
        }
    }

    pub async fn send_notification(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), std::io::Error> {
        let msg = RawMessage {
            jsonrpc: "2.0".into(),
            id: None,
            method: Some(method.into()),
            params,
            result: None,
            error: None,
        };
        self.write_message(&msg).await
    }

    pub async fn send_request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<RequestId, std::io::Error> {
        let id = next_request_id();
        let msg = RawMessage {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(id))),
            method: Some(method.into()),
            params,
            result: None,
            error: None,
        };
        self.write_message(&msg).await.map(|_| id)
    }

    pub async fn send_response(
        &mut self,
        id: RequestId,
        result: Option<serde_json::Value>,
        error: Option<JsonRpcError>,
    ) -> Result<(), std::io::Error> {
        let msg = RawMessage {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(id))),
            method: None,
            params: None,
            result,
            error,
        };
        self.write_message(&msg).await
    }

    async fn write_message(&mut self, msg: &RawMessage) -> Result<(), std::io::Error> {
        let body = serde_json::to_string(msg).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e)
        })?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        self.inner.write_all(header.as_bytes()).await?;
        self.inner.write_all(body.as_bytes()).await?;
        self.inner.flush().await?;
        Ok(())
    }
}

pub struct RpcReader {
    inner: BufReader<ChildStdout>,
}

impl RpcReader {
    pub fn new(stdout: ChildStdout) -> Self {
        Self {
            inner: BufReader::new(stdout),
        }
    }

    pub async fn read_message(&mut self) -> Result<Option<RpcMessage>, std::io::Error> {
        let content_length = match self.read_headers().await? {
            Some(len) => len,
            None => return Ok(None),
        };
        let mut body = vec![0u8; content_length];
        self.inner.read_exact(&mut body).await?;
        let raw: RawMessage = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to parse JSON-RPC message: {}", e);
                let text = String::from_utf8_lossy(&body);
                tracing::debug!(%text, "Raw message body");
                return Ok(None);
            }
        };
        Ok(Some(raw.into_message()))
    }

    async fn read_headers(&mut self) -> Result<Option<usize>, std::io::Error> {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            let n = self.inner.read_line(&mut line).await?;
            if n == 0 {
                return Ok(None);
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(len) = trimmed.strip_prefix("Content-Length: ") {
                content_length = Some(len.trim().parse::<usize>().map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                })?);
            }
        }
        Ok(content_length)
    }
}

pub struct RpcTransport {
    pub reader: RpcReader,
    pub writer: RpcWriter,
}

impl RpcTransport {
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        Self {
            reader: RpcReader::new(stdout),
            writer: RpcWriter::new(stdin),
        }
    }
}

pub async fn read_stderr(mut stderr: ChildStderr, language: String) {
    let mut buf = String::new();
    let mut reader = tokio::io::BufReader::new(&mut stderr);
    loop {
        buf.clear();
        match reader.read_line(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                tracing::debug!(language = %language, stderr = %buf.trim(), "Server stderr");
            }
        }
    }
}
