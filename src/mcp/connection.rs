/// MCP 连接接口
///
/// 定义了与 MCP 服务器通信的标准接口

use super::types::{McpRequest, McpResponse, McpToolDefinition};
use anyhow::Result;
use async_trait::async_trait;

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// 未连接
    Disconnected,

    /// 连接中
    Connecting,

    /// 已连接
    Connected,

    /// 错误
    Error,
}

/// MCP 连接 trait
///
/// 所有 MCP 连接实现都必须实现此 trait
#[async_trait]
pub trait McpConnection: Send + Sync {
    /// 获取服务器名称
    fn server_name(&self) -> &str;

    /// 获取连接状态
    fn status(&self) -> ConnectionStatus;

    /// 连接到服务器
    async fn connect(&mut self) -> Result<()>;

    /// 断开连接
    async fn disconnect(&mut self) -> Result<()>;

    /// 发送请求并接收响应
    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse>;

    /// 列出可用工具
    async fn list_tools(&mut self) -> Result<Vec<McpToolDefinition>> {
        let request = McpRequest::list_tools(self.next_request_id());
        let response = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("MCP error: {}", error.message));
        }

        let result = response.result.ok_or_else(|| {
            anyhow::anyhow!("No result in tools/list response")
        })?;

        // 解析工具列表
        let tools: Vec<McpToolDefinition> = serde_json::from_value(
            result.get("tools")
                .ok_or_else(|| anyhow::anyhow!("No 'tools' field in response"))?
                .clone()
        )?;

        Ok(tools)
    }

    /// 调用工具
    async fn call_tool(
        &mut self,
        name: String,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let request = McpRequest::call_tool(self.next_request_id(), name.clone(), arguments);
        let response = self.send_request(request).await?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!("MCP error calling {}: {}", name, error.message));
        }

        response.result.ok_or_else(|| {
            anyhow::anyhow!("No result in tools/call response")
        })
    }

    /// 获取下一个请求 ID
    fn next_request_id(&self) -> u64;

    /// 健康检查
    async fn health_check(&mut self) -> Result<bool> {
        // 默认实现：尝试列出工具
        match self.list_tools().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试 ConnectionStatus
    #[test]
    fn test_connection_status() {
        assert_eq!(ConnectionStatus::Disconnected, ConnectionStatus::Disconnected);
        assert_ne!(ConnectionStatus::Connected, ConnectionStatus::Disconnected);
    }
}
