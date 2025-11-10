/// MCP 工具适配器
///
/// 将 MCP 工具定义转换为我们的 ToolSpec 格式，并创建可执行的 Tool 实例
use super::connection::McpConnection;
use super::types::McpToolDefinition;
use crate::tools::spec::{Tool, ToolOutput, ToolSpec};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// MCP 工具适配器
///
/// 将 MCP 工具定义转换为我们的工具系统
pub struct McpToolAdapter;

impl McpToolAdapter {
    /// 将 MCP 工具定义转换为 ToolSpec
    pub fn to_tool_spec(server_name: &str, mcp_tool: &McpToolDefinition) -> ToolSpec {
        // 生成限定名: mcp__{server}__{tool}
        let qualified_name = Self::qualified_name(server_name, &mcp_tool.name);

        let description = if mcp_tool.description.is_empty() {
            format!("MCP tool {} from server {}", mcp_tool.name, server_name)
        } else {
            mcp_tool.description.clone()
        };

        ToolSpec {
            tool_type: "function".to_string(),
            name: qualified_name,
            description: Some(description),
            parameters: mcp_tool.input_schema.clone(),
        }
    }

    /// 生成限定名
    ///
    /// 格式: mcp__{server}__{tool}
    pub fn qualified_name(server_name: &str, tool_name: &str) -> String {
        format!("mcp__{}__{}", server_name, tool_name)
    }

    /// 从限定名解析服务器和工具名
    ///
    /// 返回: (server_name, tool_name)
    pub fn parse_qualified_name(qualified_name: &str) -> Option<(String, String)> {
        if !qualified_name.starts_with("mcp__") {
            return None;
        }

        let rest = &qualified_name[5..]; // 跳过 "mcp__"
        let parts: Vec<&str> = rest.split("__").collect();

        if parts.len() != 2 {
            return None;
        }

        Some((parts[0].to_string(), parts[1].to_string()))
    }

    /// 创建 MCP 工具实例
    pub fn create_tool(
        server_name: String,
        mcp_tool: McpToolDefinition,
        connection: Arc<Mutex<dyn McpConnection>>,
    ) -> Arc<dyn Tool> {
        let spec = Self::to_tool_spec(&server_name, &mcp_tool);
        let tool_name = mcp_tool.name.clone();

        Arc::new(McpTool {
            server_name,
            tool_name,
            spec,
            connection,
        })
    }
}

/// MCP 工具实现
///
/// 通过 MCP 连接调用远程工具
struct McpTool {
    server_name: String,
    tool_name: String,
    spec: ToolSpec,
    connection: Arc<Mutex<dyn McpConnection>>,
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.spec.name
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    async fn execute(&self, args: Value) -> Result<ToolOutput> {
        println!(
            "🔧 调用 MCP 工具: {} (服务器: {})",
            self.tool_name, self.server_name
        );

        // 通过连接调用工具
        let mut conn = self.connection.lock().await;
        let result = conn.call_tool(self.tool_name.clone(), args).await?;

        // 解析结果
        // MCP 响应格式: { "content": [...] }
        let content = result
            .get("content")
            .ok_or_else(|| anyhow::anyhow!("No 'content' in MCP tool response"))?;

        // 提取文本内容
        let text = Self::extract_text_from_content(content)?;

        Ok(ToolOutput::Text(text))
    }

    fn requires_approval(&self) -> bool {
        // 默认情况下，所有 MCP 工具都需要审批
        // 可以根据配置或工具名称来决定
        true
    }
}

impl McpTool {
    /// 从 MCP content 数组中提取文本
    fn extract_text_from_content(content: &Value) -> Result<String> {
        let content_array = content
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Expected 'content' to be an array"))?;

        let mut text_parts = Vec::new();

        for item in content_array {
            if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                if item_type == "text" {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
                // TODO: 支持其他类型（image, resource）
            }
        }

        if text_parts.is_empty() {
            Ok(String::new())
        } else {
            Ok(text_parts.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qualified_name() {
        let name = McpToolAdapter::qualified_name("filesystem", "read_file");
        assert_eq!(name, "mcp__filesystem__read_file");
    }

    #[test]
    fn test_parse_qualified_name() {
        let (server, tool) = McpToolAdapter::parse_qualified_name("mcp__filesystem__read_file")
            .expect("Should parse");

        assert_eq!(server, "filesystem");
        assert_eq!(tool, "read_file");
    }

    #[test]
    fn test_parse_qualified_name_invalid() {
        assert!(McpToolAdapter::parse_qualified_name("not_mcp_tool").is_none());
        assert!(McpToolAdapter::parse_qualified_name("mcp__only_one_part").is_none());
    }

    #[test]
    fn test_to_tool_spec() {
        let mcp_tool = McpToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                }
            }),
        };

        let spec = McpToolAdapter::to_tool_spec("filesystem", &mcp_tool);

        assert_eq!(spec.name, "mcp__filesystem__read_file");
        assert_eq!(spec.description, Some("Read a file".to_string()));
    }

    #[test]
    fn test_extract_text_from_content() {
        let content = serde_json::json!([
            {"type": "text", "text": "Hello"},
            {"type": "text", "text": "World"}
        ]);

        let text = McpTool::extract_text_from_content(&content).unwrap();
        assert_eq!(text, "Hello\nWorld");
    }

    #[test]
    fn test_extract_text_empty() {
        let content = serde_json::json!([]);
        let text = McpTool::extract_text_from_content(&content).unwrap();
        assert_eq!(text, "");
    }
}
