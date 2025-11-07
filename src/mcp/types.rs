/// MCP 类型定义
///
/// 定义 MCP 协议中使用的核心数据结构

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// 服务器名称（用于限定名）
    pub name: String,

    /// 连接类型
    pub connection_type: ConnectionType,

    /// 连接参数
    pub connection_params: ConnectionParams,

    /// 工具过滤器
    #[serde(default)]
    pub tool_filter: ToolFilter,

    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// 连接类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConnectionType {
    /// stdio 进程连接
    #[serde(rename = "stdio")]
    Stdio,

    /// HTTP/SSE 连接
    #[serde(rename = "http")]
    Http,
}

/// 连接参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConnectionParams {
    /// stdio 参数
    Stdio {
        /// 命令
        command: String,

        /// 参数
        #[serde(default)]
        args: Vec<String>,

        /// 环境变量
        #[serde(default)]
        env: HashMap<String, String>,
    },

    /// HTTP 参数
    Http {
        /// 基础 URL
        url: String,

        /// 认证头
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// 工具过滤器
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolFilter {
    /// 允许列表（如果为空则允许全部）
    #[serde(default)]
    pub allow: Vec<String>,

    /// 拒绝列表
    #[serde(default)]
    pub deny: Vec<String>,
}

impl ToolFilter {
    /// 检查工具是否被允许
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        // 如果在拒绝列表中，直接拒绝
        if self.deny.iter().any(|pattern| Self::matches(tool_name, pattern)) {
            return false;
        }

        // 如果允许列表为空，则允许全部（除了被拒绝的）
        if self.allow.is_empty() {
            return true;
        }

        // 检查是否在允许列表中
        self.allow.iter().any(|pattern| Self::matches(tool_name, pattern))
    }

    /// 简单的模式匹配（支持 * 通配符）
    fn matches(text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') {
            // 简化实现：仅支持前缀和后缀匹配
            if pattern.starts_with('*') && pattern.ends_with('*') {
                let content = &pattern[1..pattern.len() - 1];
                text.contains(content)
            } else if pattern.starts_with('*') {
                let suffix = &pattern[1..];
                text.ends_with(suffix)
            } else if pattern.ends_with('*') {
                let prefix = &pattern[..pattern.len() - 1];
                text.starts_with(prefix)
            } else {
                text == pattern
            }
        } else {
            text == pattern
        }
    }
}

/// MCP 工具定义
///
/// 从 MCP 服务器获取的工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// 工具名称
    pub name: String,

    /// 描述
    #[serde(default)]
    pub description: String,

    /// 参数 schema（JSON Schema）
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// MCP RPC 请求
#[derive(Debug, Clone, Serialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// 创建新请求
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.into(),
            params,
        }
    }

    /// 列出工具
    pub fn list_tools(id: u64) -> Self {
        Self::new(id, "tools/list", None)
    }

    /// 调用工具
    pub fn call_tool(id: u64, name: String, arguments: serde_json::Value) -> Self {
        Self::new(
            id,
            "tools/call",
            Some(serde_json::json!({
                "name": name,
                "arguments": arguments,
            })),
        )
    }
}

/// MCP RPC 响应
#[derive(Debug, Clone, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<McpError>,
}

/// MCP 错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_filter_allow_all() {
        let filter = ToolFilter::default();
        assert!(filter.is_allowed("any_tool"));
        assert!(filter.is_allowed("another_tool"));
    }

    #[test]
    fn test_tool_filter_allowlist() {
        let filter = ToolFilter {
            allow: vec!["read_file".to_string(), "write_file".to_string()],
            deny: vec![],
        };

        assert!(filter.is_allowed("read_file"));
        assert!(filter.is_allowed("write_file"));
        assert!(!filter.is_allowed("delete_file"));
    }

    #[test]
    fn test_tool_filter_denylist() {
        let filter = ToolFilter {
            allow: vec![],
            deny: vec!["delete_file".to_string(), "format_disk".to_string()],
        };

        assert!(filter.is_allowed("read_file"));
        assert!(!filter.is_allowed("delete_file"));
        assert!(!filter.is_allowed("format_disk"));
    }

    #[test]
    fn test_tool_filter_wildcard() {
        let filter = ToolFilter {
            allow: vec!["read_*".to_string()],
            deny: vec!["*_dangerous".to_string()],
        };

        assert!(filter.is_allowed("read_file"));
        assert!(filter.is_allowed("read_config"));
        assert!(!filter.is_allowed("write_file"));
        assert!(!filter.is_allowed("read_dangerous"));
    }

    #[test]
    fn test_mcp_request_serialization() {
        let req = McpRequest::list_tools(1);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("tools/list"));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_mcp_request_call_tool() {
        let req = McpRequest::call_tool(
            2,
            "read_file".to_string(),
            serde_json::json!({"path": "/tmp/test.txt"}),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("tools/call"));
        assert!(json.contains("read_file"));
    }
}
