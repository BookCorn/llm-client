/// Responses API 事件抽象层
///
/// 基于 Codex-CLI 的事件驱动架构设计，提供类型安全的 SSE 事件处理
///
/// 参考：
/// - codex-rs/core/src/client_common.rs:197 (ResponseEvent)
/// - codex-rs/protocol/src/models.rs (ResponseItem)
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Responses API 的核心事件枚举
///
/// 对应 OpenAI Responses API 的 SSE 事件流
/// 参考文档第89-110行
#[derive(Clone, Debug)]
pub enum ResponseEvent {
    /// response.created - 响应开始
    Created { response_id: String },

    /// response.output_item.done - 完整的响应项产出
    /// 这是"边产出边处理"的关键事件，允许即时消费工具调用
    OutputItemDone(ResponseItem),

    /// response.output_item.added - 新项添加（较少使用）
    OutputItemAdded(ResponseItem),

    /// response.output_text.delta - 助手文本流式增量
    OutputTextDelta(String),

    /// response.reasoning_summary_text.delta - 推理摘要流式增量
    ReasoningSummaryDelta(String),

    /// response.reasoning_text.delta - 详细推理内容（raw reasoning）
    ReasoningContentDelta(String),

    /// response.reasoning_summary_part.added - 推理摘要小节边界
    /// 用于分段显示（如 "Plan / Actions / Checks"）
    ReasoningSummaryPartAdded,

    /// response.completed - 响应完成
    Completed {
        response_id: String,
        token_usage: Option<TokenUsage>,
    },

    /// 速率限制信息（从响应头解析）
    RateLimits(RateLimitSnapshot),

    /// response.failed - 响应失败
    Failed {
        error: String,
        retry_after: Option<u64>, // 秒数
    },
}

/// 响应项 - 对应一个完整的输出单元
///
/// Responses API 可以返回多种类型的项：消息、推理、工具调用等
/// 参考文档第101-103行, 210-217行
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseItem {
    /// 普通消息（助手回复）
    Message {
        role: String,
        content: Vec<ContentBlock>,
    },

    /// 推理项（推理模型的思考过程）
    Reasoning {
        /// 推理摘要（用户可见的思考总结）
        summary: Vec<ReasoningText>,
        /// 详细推理内容（可选，更详细的思考过程）
        content: Option<Vec<ReasoningText>>,
        /// 加密内容（某些供应商支持）
        encrypted_content: Option<String>,
    },

    /// 函数调用（工具调用请求）
    FunctionCall {
        call_id: String,
        name: String,
        /// ⚠️ 注意：在 Responses API 中，arguments 是"字符串化的 JSON"
        /// 需要再次解析：serde_json::from_str(&arguments)
        arguments: String,
    },

    /// 本地 Shell 调用（特殊的工具类型）
    LocalShellCall { call_id: String, command: String },

    /// 自定义工具调用（扩展点）
    CustomToolCall {
        call_id: String,
        tool_name: String,
        arguments: String,
    },

    /// 函数调用输出（工具执行结果）
    FunctionCallOutput {
        call_id: String,
        output: FunctionCallOutputPayload,
    },
}

/// 内容块 - 支持文本和富媒体
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    #[serde(rename = "output_text")]
    Text { text: String },

    #[serde(rename = "output_image")]
    Image { image_url: String },

    #[serde(rename = "input_text")]
    InputText { text: String },

    #[serde(rename = "input_image")]
    InputImage { image_url: String },
}

/// 推理文本块
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReasoningText {
    #[serde(rename = "summary_text")]
    SummaryText { text: String },

    #[serde(rename = "reasoning_text")]
    ReasoningText { text: String },
}

/// 函数调用输出载荷
///
/// 支持两种格式：
/// 1. 纯文本（简单场景）
/// 2. 结构化内容（包含图片等富媒体）
///
/// 参考文档第219-230行, 256-262行
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FunctionCallOutputPayload {
    /// 纯文本输出（最常见）
    Text(String),

    /// 结构化输出（支持富媒体）
    Structured { content_items: Vec<ContentBlock> },
}

/// Token 使用统计
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub reasoning_tokens: u64,
}

/// 速率限制快照
///
/// 从响应头解析两级窗口数据
/// 参考文档第286-289行
#[derive(Clone, Debug)]
pub struct RateLimitSnapshot {
    pub requests: WindowInfo,
    pub tokens: WindowInfo,
}

/// 速率限制窗口信息
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub limit: u64,
    pub remaining: u64,
    pub reset_at: std::time::SystemTime,
}

/// SSE 事件解析器
///
/// 负责将原始 SSE 事件映射到类型安全的 ResponseEvent
pub struct EventParser {
    current_event_type: String,
    saw_completed: bool,
    response_id: Option<String>,
}

impl EventParser {
    pub fn new() -> Self {
        Self {
            current_event_type: String::new(),
            saw_completed: false,
            response_id: None,
        }
    }

    /// 设置当前事件类型（从 "event: " 行读取）
    pub fn set_event_type(&mut self, event_type: String) {
        self.current_event_type = event_type;
    }

    /// 解析数据行（从 "data: " 行读取）
    ///
    /// 参考文档第99-166行
    pub fn parse_data(&mut self, data: &str) -> anyhow::Result<Option<ResponseEvent>> {
        // [DONE] 标记
        if data == "[DONE]" {
            return Ok(None);
        }

        // 解析 JSON
        let json: Value = serde_json::from_str(data)?;

        // 根据事件类型映射
        let event = match self.current_event_type.as_str() {
            "response.created" => {
                let response_id = json["response"]["id"].as_str().unwrap_or("").to_string();
                self.response_id = Some(response_id.clone());
                Some(ResponseEvent::Created { response_id })
            }

            "response.output_item.done" => {
                // 解析完整的 ResponseItem
                match json.get("item") {
                    Some(value) => match serde_json::from_value::<ResponseItem>(value.clone()) {
                        Ok(item) => Some(ResponseEvent::OutputItemDone(item)),
                        Err(err) => {
                            println!("⚠️ 无法解析 output_item.done: {}", err);
                            None
                        }
                    },
                    None => {
                        println!("⚠️ output_item.done 事件缺少 item 字段");
                        None
                    }
                }
            }

            "response.output_item.added" => match json.get("item") {
                Some(value) => match serde_json::from_value::<ResponseItem>(value.clone()) {
                    Ok(item) => Some(ResponseEvent::OutputItemAdded(item)),
                    Err(err) => {
                        println!("⚠️ 无法解析 output_item.added: {}", err);
                        None
                    }
                },
                None => {
                    println!("⚠️ output_item.added 事件缺少 item 字段");
                    None
                }
            },

            "response.output_text.delta" => {
                let delta = json
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(ResponseEvent::OutputTextDelta(delta))
            }

            "response.reasoning_summary_text.delta" => {
                let delta = json
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(ResponseEvent::ReasoningSummaryDelta(delta))
            }

            "response.reasoning_text.delta" => {
                let delta = json
                    .get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(ResponseEvent::ReasoningContentDelta(delta))
            }

            "response.reasoning_summary_part.added" => {
                Some(ResponseEvent::ReasoningSummaryPartAdded)
            }

            "response.completed" => {
                self.saw_completed = true;
                let response_id = nested_str(&json, &["response", "id"])
                    .unwrap_or("")
                    .to_string();

                // 解析 token 使用情况
                let token_usage = nested(&json, &["response", "usage"])
                    .and_then(|usage| usage.as_object())
                    .map(|usage| TokenUsage {
                        input_tokens: usage
                            .get("input_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        output_tokens: usage
                            .get("output_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                        reasoning_tokens: usage
                            .get("reasoning_tokens")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0),
                    });

                Some(ResponseEvent::Completed {
                    response_id,
                    token_usage,
                })
            }

            "response.failed" => {
                let error = nested_str(&json, &["error", "message"])
                    .unwrap_or("Unknown error")
                    .to_string();

                // 尝试解析重试建议
                let retry_after = nested(&json, &["error", "retry_after"]).and_then(|v| v.as_u64());

                Some(ResponseEvent::Failed { error, retry_after })
            }

            _ => {
                // 未知事件类型 - 仅记录日志
                println!(
                    "ℹ️ 未知事件类型: {} | 数据: {}",
                    self.current_event_type, data
                );
                None
            }
        };

        Ok(event)
    }

    /// 流结束时检查一致性
    ///
    /// 参考文档第109-111行, 273-274行
    pub fn finalize(&self) -> anyhow::Result<Option<ResponseEvent>> {
        if !self.saw_completed {
            return Err(anyhow::anyhow!(
                "Stream closed before response.completed event"
            ));
        }

        if let Some(response_id) = &self.response_id {
            // 流正常结束，发送最终的 Completed 事件
            Ok(Some(ResponseEvent::Completed {
                response_id: response_id.clone(),
                token_usage: None, // 已经在 response.completed 事件中发送
            }))
        } else {
            Ok(None)
        }
    }

    /// 检查是否已看到 completed 事件
    pub fn saw_completed(&self) -> bool {
        self.saw_completed
    }
}

impl Default for EventParser {
    fn default() -> Self {
        Self::new()
    }
}

/// 速率限制解析器
///
/// 从 HTTP 响应头中提取速率限制信息
/// 参考文档第286-289行
pub fn parse_rate_limit_snapshot(
    headers: &reqwest::header::HeaderMap,
) -> Option<RateLimitSnapshot> {
    let requests = WindowInfo {
        limit: headers
            .get("x-ratelimit-limit-requests")?
            .to_str()
            .ok()?
            .parse()
            .ok()?,
        remaining: headers
            .get("x-ratelimit-remaining-requests")?
            .to_str()
            .ok()?
            .parse()
            .ok()?,
        reset_at: parse_reset_time(headers.get("x-ratelimit-reset-requests")?.to_str().ok()?)?,
    };

    let tokens = WindowInfo {
        limit: headers
            .get("x-ratelimit-limit-tokens")?
            .to_str()
            .ok()?
            .parse()
            .ok()?,
        remaining: headers
            .get("x-ratelimit-remaining-tokens")?
            .to_str()
            .ok()?
            .parse()
            .ok()?,
        reset_at: parse_reset_time(headers.get("x-ratelimit-reset-tokens")?.to_str().ok()?)?,
    };

    Some(RateLimitSnapshot { requests, tokens })
}

fn parse_reset_time(reset_str: &str) -> Option<std::time::SystemTime> {
    // 尝试解析 Unix 时间戳或 ISO 8601 格式
    if let Ok(timestamp) = reset_str.parse::<u64>() {
        Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp))
    } else {
        // TODO: 添加 ISO 8601 解析
        None
    }
}

fn nested<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn nested_str<'a>(value: &'a Value, path: &[&str]) -> Option<&'a str> {
    nested(value, path)?.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_parser_created() {
        let mut parser = EventParser::new();
        parser.set_event_type("response.created".to_string());

        let data = r#"{"response":{"id":"resp_123"}}"#;
        let event = parser.parse_data(data).unwrap();

        match event {
            Some(ResponseEvent::Created { response_id }) => {
                assert_eq!(response_id, "resp_123");
            }
            _ => panic!("Expected Created event"),
        }
    }

    #[test]
    fn test_event_parser_output_text_delta() {
        let mut parser = EventParser::new();
        parser.set_event_type("response.output_text.delta".to_string());

        let data = r#"{"delta":"Hello"}"#;
        let event = parser.parse_data(data).unwrap();

        match event {
            Some(ResponseEvent::OutputTextDelta(delta)) => {
                assert_eq!(delta, "Hello");
            }
            _ => panic!("Expected OutputTextDelta event"),
        }
    }

    #[test]
    fn test_event_parser_reasoning_summary_delta() {
        let mut parser = EventParser::new();
        parser.set_event_type("response.reasoning_summary_text.delta".to_string());

        let data = r#"{"delta":"Thinking..."}"#;
        let event = parser.parse_data(data).unwrap();

        match event {
            Some(ResponseEvent::ReasoningSummaryDelta(delta)) => {
                assert_eq!(delta, "Thinking...");
            }
            _ => panic!("Expected ReasoningSummaryDelta event"),
        }
    }

    #[test]
    fn test_event_parser_finalize_error() {
        let parser = EventParser::new();
        // 未看到 completed 事件
        let result = parser.finalize();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("before response.completed")
        );
    }
}
