/// 工具路由器
///
/// 负责将模型的工具调用请求路由到对应的工具实现
///
/// 参考 codex-rs/core/src/tools/router.rs
use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::services::events::ResponseItem;

use super::spec::ToolInvocation;

/// 工具路由器
pub struct ToolRouter;

impl ToolRouter {
    /// 从 ResponseItem 构建 ToolInvocation
    ///
    /// # 参数
    /// - `item`: 从 SSE 流中解析的 ResponseItem
    ///
    /// # 返回
    /// - `Ok(Some(ToolInvocation))`: 成功解析工具调用
    /// - `Ok(None)`: 不是工具调用项
    /// - `Err`: 解析失败（缺少必需字段等）
    ///
    /// # 参考
    /// - codex-rs/core/src/tools/router.rs:37, 68
    /// - 文档第196-199行
    pub fn build_tool_invocation(item: ResponseItem) -> Result<Option<ToolInvocation>> {
        match item {
            ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
            } => {
                // ⚠️ 注意：在 Responses API 中，arguments 是"字符串化的 JSON"
                // 需要再次反序列化
                let args: Value = serde_json::from_str(&arguments).map_err(|e| {
                    anyhow!(
                        "Failed to parse function call arguments: {} | raw: {}",
                        e,
                        arguments
                    )
                })?;

                Ok(Some(ToolInvocation::new(call_id, name, args)))
            }

            ResponseItem::LocalShellCall { call_id, command } => {
                // 本地 Shell 调用：转换为标准的 ToolInvocation
                let args = serde_json::json!({
                    "command": command
                });

                Ok(Some(ToolInvocation::new(
                    call_id,
                    "local_shell".to_string(),
                    args,
                )))
            }

            ResponseItem::CustomToolCall {
                call_id,
                tool_name,
                arguments,
            } => {
                // 自定义工具调用
                let args: Value = serde_json::from_str(&arguments).map_err(|e| {
                    anyhow!(
                        "Failed to parse custom tool arguments: {} | raw: {}",
                        e,
                        arguments
                    )
                })?;

                Ok(Some(ToolInvocation::new(call_id, tool_name, args)))
            }

            // 非工具调用项
            _ => Ok(None),
        }
    }

    /// 验证 call_id 是否存在
    ///
    /// # 参考
    /// - codex-rs/core/src/tools/router.rs:108
    /// - 文档第383-384行
    pub fn validate_call_id(call_id: &str) -> Result<()> {
        if call_id.is_empty() {
            return Err(anyhow!("Tool call missing call_id"));
        }
        Ok(())
    }

    /// 解析 MCP 工具名
    ///
    /// MCP 工具使用限定名格式：mcp__{server}__{tool}
    ///
    /// # 示例
    /// ```
    /// let (server, tool) = parse_mcp_tool_name("mcp__docs__search")?;
    /// assert_eq!(server, "docs");
    /// assert_eq!(tool, "search");
    /// ```
    ///
    /// # 参考
    /// - codex-rs/core/src/codex.rs:1232
    /// - 文档第241-242行, 391行
    pub fn parse_mcp_tool_name(name: &str) -> Result<(String, String)> {
        if !name.starts_with("mcp__") {
            return Err(anyhow!("Not an MCP tool: {}", name));
        }

        let parts: Vec<&str> = name.strip_prefix("mcp__").unwrap().split("__").collect();

        if parts.len() < 2 {
            return Err(anyhow!("Invalid MCP tool name format: {}", name));
        }

        Ok((parts[0].to_string(), parts[1..].join("__")))
    }

    /// 检查是否是 MCP 工具
    pub fn is_mcp_tool(name: &str) -> bool {
        name.starts_with("mcp__")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mcp_tool_name() {
        let (server, tool) = ToolRouter::parse_mcp_tool_name("mcp__docs__search").unwrap();
        assert_eq!(server, "docs");
        assert_eq!(tool, "search");

        let (server, tool) = ToolRouter::parse_mcp_tool_name("mcp__filesystem__read_file").unwrap();
        assert_eq!(server, "filesystem");
        assert_eq!(tool, "read_file");
    }

    #[test]
    fn test_parse_mcp_tool_name_invalid() {
        assert!(ToolRouter::parse_mcp_tool_name("not_mcp_tool").is_err());
        assert!(ToolRouter::parse_mcp_tool_name("mcp__").is_err());
        assert!(ToolRouter::parse_mcp_tool_name("mcp__only_one_part").is_err());
    }

    #[test]
    fn test_is_mcp_tool() {
        assert!(ToolRouter::is_mcp_tool("mcp__docs__search"));
        assert!(!ToolRouter::is_mcp_tool("local_shell"));
        assert!(!ToolRouter::is_mcp_tool("shell"));
    }

    #[test]
    fn test_validate_call_id() {
        assert!(ToolRouter::validate_call_id("call_123").is_ok());
        assert!(ToolRouter::validate_call_id("").is_err());
    }

    #[test]
    fn test_build_tool_invocation_function_call() {
        let item = ResponseItem::FunctionCall {
            call_id: "call_123".to_string(),
            name: "shell".to_string(),
            arguments: r#"{"command":"ls -la"}"#.to_string(),
        };

        let invocation = ToolRouter::build_tool_invocation(item).unwrap().unwrap();
        assert_eq!(invocation.call_id, "call_123");
        assert_eq!(invocation.name, "shell");
        assert_eq!(invocation.arguments["command"], "ls -la");
    }

    #[test]
    fn test_build_tool_invocation_local_shell() {
        let item = ResponseItem::LocalShellCall {
            call_id: "call_456".to_string(),
            command: "pwd".to_string(),
        };

        let invocation = ToolRouter::build_tool_invocation(item).unwrap().unwrap();
        assert_eq!(invocation.call_id, "call_456");
        assert_eq!(invocation.name, "local_shell");
        assert_eq!(invocation.arguments["command"], "pwd");
    }
}
