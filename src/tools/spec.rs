/// 工具规范定义
///
/// 参考 codex-rs/core/src/tools/spec.rs

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 工具规范 - 描述工具的元数据
///
/// 对应 Responses API 的 tools 字段格式
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolSpec {
    /// 工具类型（通常为 "function"）
    #[serde(rename = "type")]
    pub tool_type: String,

    /// 工具名称（必须符合 ^[a-zA-Z0-9_-]+$ 正则）
    pub name: String,

    /// 工具描述（向模型解释工具用途）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// 参数 JSON Schema
    pub parameters: Value,
}

impl ToolSpec {
    /// 创建函数类型的工具规范
    pub fn function(name: String, description: String, parameters: Value) -> Self {
        Self {
            tool_type: "function".to_string(),
            name,
            description: Some(description),
            parameters,
        }
    }

    /// 转换为 Responses API 的 JSON 格式
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

/// 工具输出
#[derive(Clone, Debug)]
pub enum ToolOutput {
    /// 纯文本输出（最常见）
    Text(String),

    /// 结构化输出（支持富媒体）
    Structured {
        text: String,
        images: Vec<String>, // data URLs
    },

    /// 错误输出
    Error(String),
}

impl ToolOutput {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// 序列化为 Responses API 的 function_call_output 格式
    pub fn to_json(&self) -> Value {
        match self {
            Self::Text(text) => {
                serde_json::json!(text)
            }
            Self::Structured { text, images } => {
                let mut content_items = vec![serde_json::json!({
                    "type": "input_text",
                    "text": text
                })];

                for image_url in images {
                    content_items.push(serde_json::json!({
                        "type": "input_image",
                        "image_url": image_url
                    }));
                }

                serde_json::json!({
                    "content_items": content_items
                })
            }
            Self::Error(msg) => {
                serde_json::json!(format!("Error: {}", msg))
            }
        }
    }
}

/// 工具 trait
///
/// 所有工具必须实现此 trait
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称
    fn name(&self) -> &str;

    /// 工具规范（用于生成 API 请求）
    fn spec(&self) -> ToolSpec;

    /// 执行工具
    ///
    /// # 参数
    /// - `args`: 工具参数（JSON 对象）
    ///
    /// # 返回
    /// - `Ok(ToolOutput)`: 执行成功
    /// - `Err(anyhow::Error)`: 执行失败
    async fn execute(&self, args: Value) -> Result<ToolOutput>;

    /// 是否需要用户审批（默认：不需要）
    fn requires_approval(&self) -> bool {
        false
    }

    /// 获取审批提示（用于向用户展示）
    fn approval_prompt(&self, args: &Value) -> String {
        format!("允许执行工具 '{}' 吗？\n参数: {}", self.name(), args)
    }
}

/// 工具调用请求
///
/// 从 ResponseItem::FunctionCall 解析而来
#[derive(Clone, Debug)]
pub struct ToolInvocation {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

impl ToolInvocation {
    pub fn new(call_id: String, name: String, arguments: Value) -> Self {
        Self {
            call_id,
            name,
            arguments,
        }
    }
}

/// 工具调用结果
///
/// 将被序列化为 ResponseInputItem::FunctionCallOutput
#[derive(Clone, Debug)]
pub struct ToolResult {
    pub call_id: String,
    pub output: ToolOutput,
}

impl ToolResult {
    pub fn new(call_id: String, output: ToolOutput) -> Self {
        Self { call_id, output }
    }

    /// 转换为 Responses API 的 function_call_output 格式
    pub fn to_response_input_item(&self) -> Value {
        serde_json::json!({
            "type": "function_call_output",
            "call_id": self.call_id,
            "output": self.output.to_json()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_spec_to_json() {
        let spec = ToolSpec::function(
            "test_tool".to_string(),
            "A test tool".to_string(),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                },
                "required": ["input"]
            }),
        );

        let json = spec.to_json();
        assert_eq!(json["type"], "function");
        assert_eq!(json["name"], "test_tool");
    }

    #[test]
    fn test_tool_output_text() {
        let output = ToolOutput::text("Hello");
        let json = output.to_json();
        assert_eq!(json, serde_json::json!("Hello"));
    }

    #[test]
    fn test_tool_output_structured() {
        let output = ToolOutput::Structured {
            text: "Result".to_string(),
            images: vec!["data:image/png;base64,abc".to_string()],
        };
        let json = output.to_json();
        assert!(json["content_items"].is_array());
        assert_eq!(json["content_items"][0]["type"], "input_text");
        assert_eq!(json["content_items"][1]["type"], "input_image");
    }

    #[test]
    fn test_tool_result_to_response_input_item() {
        let result = ToolResult::new(
            "call_123".to_string(),
            ToolOutput::text("Success"),
        );
        let json = result.to_response_input_item();
        assert_eq!(json["type"], "function_call_output");
        assert_eq!(json["call_id"], "call_123");
    }
}
