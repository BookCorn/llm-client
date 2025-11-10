/// HTTP/SSE 连接实现
///
/// 通过 HTTP 请求与 MCP 服务器通信
use super::connection::{ConnectionStatus, McpConnection};
use super::types::{McpRequest, McpResponse};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// HTTP 连接
pub struct HttpConnection {
    /// 服务器名称
    server_name: String,

    /// 基础 URL
    base_url: String,

    /// 认证头
    headers: HashMap<String, String>,

    /// 连接状态
    status: ConnectionStatus,

    /// HTTP 客户端
    client: Client,

    /// 请求 ID 计数器
    request_id: AtomicU64,

    /// 超时配置
    timeout: Duration,
}

impl HttpConnection {
    /// 创建新的 HTTP 连接
    pub fn new(
        server_name: String,
        base_url: String,
        headers: HashMap<String, String>,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            server_name,
            base_url,
            headers,
            status: ConnectionStatus::Disconnected,
            client,
            request_id: AtomicU64::new(1),
            timeout: Duration::from_secs(30),
        })
    }

    /// 设置超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 构建请求 URL
    fn build_url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{}/{}", base, path)
    }

    /// 发送 JSON-RPC 请求
    async fn send_jsonrpc(&self, request: McpRequest) -> Result<McpResponse> {
        let url = self.build_url("/mcp");
        let json_body = serde_json::to_string(&request)?;

        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(json_body);

        // 添加自定义头
        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "HTTP 请求失败: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let response_text = response.text().await?;
        let mcp_response: McpResponse = serde_json::from_str(&response_text)
            .map_err(|e| anyhow!("解析 MCP 响应失败: {} - 响应: {}", e, response_text))?;

        Ok(mcp_response)
    }
}

#[async_trait]
impl McpConnection for HttpConnection {
    fn server_name(&self) -> &str {
        &self.server_name
    }

    fn status(&self) -> ConnectionStatus {
        self.status
    }

    async fn connect(&mut self) -> Result<()> {
        if self.status == ConnectionStatus::Connected {
            return Ok(());
        }

        self.status = ConnectionStatus::Connecting;

        // 测试连接：发送一个简单的请求
        let test_request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: 0,
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "1.0",
                "capabilities": {}
            })),
        };

        match self.send_jsonrpc(test_request).await {
            Ok(_) => {
                self.status = ConnectionStatus::Connected;
                println!("✅ 已连接到 HTTP MCP 服务器: {}", self.server_name);
                Ok(())
            }
            Err(e) => {
                self.status = ConnectionStatus::Error;
                Err(anyhow!(
                    "连接到 HTTP MCP 服务器 '{}' 失败: {}",
                    self.server_name,
                    e
                ))
            }
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.status = ConnectionStatus::Disconnected;
        println!("🔌 已断开 HTTP MCP 服务器: {}", self.server_name);
        Ok(())
    }

    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse> {
        if self.status != ConnectionStatus::Connected {
            return Err(anyhow!("未连接到 HTTP MCP 服务器 '{}'", self.server_name));
        }

        self.send_jsonrpc(request).await
    }

    fn next_request_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn health_check(&mut self) -> Result<bool> {
        if self.status != ConnectionStatus::Connected {
            return Ok(false);
        }

        // 尝试列出工具来验证连接
        match self.list_tools().await {
            Ok(_) => Ok(true),
            Err(_) => {
                self.status = ConnectionStatus::Error;
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_connection_creation() {
        let conn = HttpConnection::new(
            "test-server".to_string(),
            "http://localhost:3000".to_string(),
            HashMap::new(),
        )
        .unwrap();

        assert_eq!(conn.server_name(), "test-server");
        assert_eq!(conn.status(), ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_build_url() {
        let conn = HttpConnection::new(
            "test".to_string(),
            "http://localhost:3000/".to_string(),
            HashMap::new(),
        )
        .unwrap();

        assert_eq!(conn.build_url("/mcp"), "http://localhost:3000/mcp");
        assert_eq!(conn.build_url("mcp"), "http://localhost:3000/mcp");
    }

    #[test]
    fn test_request_id_increment() {
        let conn = HttpConnection::new(
            "test".to_string(),
            "http://localhost:3000".to_string(),
            HashMap::new(),
        )
        .unwrap();

        assert_eq!(conn.next_request_id(), 1);
        assert_eq!(conn.next_request_id(), 2);
        assert_eq!(conn.next_request_id(), 3);
    }
}
