use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use futures::StreamExt;
use serde_json::json;

use crate::models::Message;

/// Configuration for OpenAI API
#[derive(Clone, Debug)]
pub struct OpenAIServiceConfig {
    pub api_key: String,
    pub api_base: Option<String>,
    pub model: String,
    pub use_responses_api: bool,  // 是否使用 Responses API（第三方服务可能不支持）
    // Responses API 推理选项
    pub reasoning_effort: String,   // minimal|low|medium|high（默认：medium）
    pub reasoning_summary: String,  // auto|on|off（默认：auto）
}

impl OpenAIServiceConfig {
    /// Create config from environment variables
    ///
    /// Environment variables:
    /// - OPENAI_API_KEY (required): Your OpenAI API key
    /// - OPENAI_API_BASE (optional): Custom API endpoint (e.g., "https://api.openai.com/v1" or your own endpoint)
    /// - OPENAI_MODEL (optional): Model to use (default: "gpt-4")
    /// - OPENAI_USE_RESPONSES_API (optional): "true" 使用 Responses API, "false" 使用 Chat Completions API (default: auto-detect)
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY environment variable not set"))?;

        let api_base = std::env::var("OPENAI_API_BASE").ok();
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4".to_string());

        // 读取用户配置：显式选择是否使用 Responses API；若未设置则默认关闭
        let use_responses_api = std::env::var("OPENAI_USE_RESPONSES_API")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);

        Ok(Self {
            api_key,
            api_base,
            model,
            use_responses_api,
            reasoning_effort: std::env::var("OPENAI_REASONING_EFFORT").unwrap_or_else(|_| "medium".to_string()),
            reasoning_summary: std::env::var("OPENAI_REASONING_SUMMARY").unwrap_or_else(|_| "auto".to_string()),
        })
    }

    /// Create config with custom values
    #[allow(dead_code)]
    pub fn new(api_key: String, api_base: Option<String>, model: String) -> Self {
        Self {
            api_key,
            api_base,
            model,
            use_responses_api: true,  // 默认使用 Responses API
            reasoning_effort: "medium".to_string(),
            reasoning_summary: "auto".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct OpenAIService {
    client: Client<OpenAIConfig>,
    config: OpenAIServiceConfig,
}

impl OpenAIService {
    /// Create a new OpenAI service with default configuration (from env vars)
    pub fn new() -> Self {
        let config = OpenAIServiceConfig::from_env().unwrap_or_else(|_| {
            // Fallback to default config if env vars not set
            OpenAIServiceConfig {
                api_key: String::new(),
                api_base: None,
                model: "gpt-4".to_string(),
                use_responses_api: false,
                reasoning_effort: "medium".to_string(),
                reasoning_summary: "auto".to_string(),
            }
        });

        Self::with_config(config)
    }

    /// 是否使用 Responses API（仅由用户配置决定）
    pub fn use_responses_api(&self) -> bool {
        self.config.use_responses_api
    }

    /// 设置是否使用 Responses API（允许运行时切换）
    pub fn set_use_responses_api(&mut self, enabled: bool) {
        self.config.use_responses_api = enabled;
    }

    /// Create a new OpenAI service with custom configuration
    pub fn with_config(config: OpenAIServiceConfig) -> Self {
        let mut openai_config = OpenAIConfig::new().with_api_key(&config.api_key);

        // Set custom API base if provided
        if let Some(api_base) = &config.api_base {
            openai_config = openai_config.with_api_base(api_base);
        }

        let client = Client::with_config(openai_config);

        Self { client, config }
    }

    /// Get the current model being used
    #[allow(dead_code)]
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Get the API base URL if set
    #[allow(dead_code)]
    pub fn api_base(&self) -> Option<&str> {
        self.config.api_base.as_deref()
    }

    /// Check if using custom endpoint
    #[allow(dead_code)]
    pub fn is_custom_endpoint(&self) -> bool {
        self.config.api_base.is_some()
    }

    pub async fn get_completion(&self, messages: &[Message]) -> Result<String> {
        let openai_messages = self.convert_messages(messages)?;

        // Create chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(openai_messages)
            .build()?;

        // Make API call
        let response = self.client.chat().create(request).await?;

        // Extract the response content
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_else(|| "No response generated.".to_string());

        Ok(content)
    }

    #[allow(dead_code)]
    pub async fn get_streaming_completion<F>(&self, messages: &[Message], mut on_chunk: F) -> Result<(String, Option<String>)>
    where
        F: FnMut(String) + Send,
    {
        let openai_messages = self.convert_messages(messages)?;

        // Create streaming chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(openai_messages)
            .build()?;

        // Make streaming API call
        let mut stream = self.client.chat().create_stream(request).await?;

        let mut full_response = String::new();
        let reasoning_summary = String::new();

        // Process the stream
        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    for choice in response.choices {
                        // 提取普通内容
                        if let Some(content) = &choice.delta.content {
                            full_response.push_str(content);
                            on_chunk(content.clone());
                        }

                        // ⚠️ 注意：async-openai 0.30.1 的类型定义中还没有 reasoning_content
                        // 但推理模型（如 gpt-4o-mini, o1 等）的 API 响应中确实包含这个字段
                        //
                        // 当前的 ChatCompletionStreamResponseDelta 只有：
                        // - content: Option<String>
                        // - function_call: Option<FunctionCallStream> (deprecated)
                        // - tool_calls: Option<Vec<ChatCompletionMessageToolCallChunk>>
                        // - role: Option<Role>
                        // - refusal: Option<String>
                        //
                        // 缺少：reasoning_content
                        //
                        // 临时解决方案：
                        // 1. 等待 async-openai 库更新支持 reasoning_content
                        // 2. 或者使用自定义的 HTTP 客户端直接解析 JSON
                        // 3. 或者 fork async-openai 添加这个字段

                        // TODO: 一旦库支持，这里应该类似这样：
                        // if let Some(reasoning) = &choice.delta.reasoning_content {
                        //     reasoning_summary.push_str(reasoning);
                        // }
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        // 返回 (内容, 推理摘要)
        // 如果没有推理摘要（reasoning_summary为空），返回None
        let reasoning_opt = if reasoning_summary.is_empty() {
            None
        } else {
            Some(reasoning_summary)
        };

        Ok((full_response, reasoning_opt))
    }

    /// 使用原生 HTTP 客户端的流式完成
    ///
    /// 根据配置自动选择：
    /// - Responses API (支持 reasoning summary)
    /// - Chat Completions API (传统API，更兼容)
    pub async fn get_streaming_completion_native<F1, F2>(
        &self,
        messages: &[Message],
        on_chunk: F1,
        on_reasoning: F2,
    ) -> Result<(String, Option<String>)>
    where
        F1: FnMut(String) + Send,
        F2: FnMut(String) + Send,
    {
        // 为了在需要时回退到 Chat Completions，可以对同一闭包多次调用，改为在内部以 &mut 方式转发
        let mut on_chunk_mut = on_chunk;
        let mut on_reasoning_mut = on_reasoning;

        if self.config.use_responses_api {
            println!("ℹ️ 使用 Responses API（支持 reasoning summary）");
            self
                .call_responses_api(messages, &mut on_chunk_mut, &mut on_reasoning_mut)
                .await
        } else {
            println!("ℹ️ 使用 Chat Completions API（传统模式，不支持 reasoning summary）");
            self.call_chat_completions_api(messages, &mut on_chunk_mut).await
        }
    }

    /// 使用 Chat Completions API
    async fn call_chat_completions_api<F>(
        &self,
        messages: &[Message],
        on_chunk: &mut F,
    ) -> Result<(String, Option<String>)>
    where
        F: FnMut(String) + Send,
    {
        use futures::StreamExt;

        // 构建请求体 - Chat Completions API 格式
        let api_messages = self.convert_messages_to_json(messages)?;
        let request_body = json!({
            "model": &self.config.model,
            "messages": api_messages,
            "stream": true,
        });

        // 构建 API base URL - Chat Completions API 端点
        let api_base = self.config.api_base.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/chat/completions", api_base);

        println!("🔗 调用 Chat Completions API: {}", url);
        println!("📝 模型: {}", &self.config.model);

        // 创建 HTTP 客户端
        let client = reqwest::Client::new();

        // 发送请求
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("API 请求失败 ({}): {}", status, error_text));
        }

        let mut full_response = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();

        // 解析 SSE 流 - Chat Completions API 格式（只有 data: 行）
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            buffer.extend_from_slice(&chunk);

            while let Some(line_end) = buffer.iter().position(|&b| b == b'\n') {
                let line = buffer[..line_end].to_vec();
                buffer.drain(..=line_end);

                let line_str = String::from_utf8_lossy(&line);
                let line_str = line_str.trim();

                if line_str.is_empty() || line_str.starts_with(':') {
                    continue;
                }

                if let Some(data) = line_str.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        println!("✅ 流式传输完成");
                        break;
                    }

                    match serde_json::from_str::<serde_json::Value>(data) {
                        Ok(json) => {
                            if let Some(choices) = json["choices"].as_array() {
                                for choice in choices {
                                    let delta = &choice["delta"];
                                    if let Some(content) = delta["content"].as_str() {
                                        full_response.push_str(content);
                                        on_chunk(content.to_string());
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️ 解析 JSON 失败: {} - 数据: {}", e, data);
                        }
                    }
                }
            }
        }

        println!("✅ Chat Completions API 完成 - 输出: {} 字符", full_response.len());
        Ok((full_response, None))  // Chat Completions API 不返回 reasoning summary
    }

    /// 使用 Responses API
    async fn call_responses_api<F1, F2>(
        &self,
        messages: &[Message],
        on_chunk: &mut F1,
        on_reasoning: &mut F2,
    ) -> Result<(String, Option<String>)>
    where
        F1: FnMut(String) + Send,
        F2: FnMut(String) + Send,
    {
        use futures::StreamExt;

        // 构建用户输入（Responses API 使用 input 而不是 messages）
        // 取最后一条“用户”消息，避免误取刚插入的空 Assistant 占位消息
        let input = messages
            .iter()
            .rev()
            .find(|m| m.is_user() && !m.content.trim().is_empty())
            .map(|m| m.content.clone())
            .ok_or_else(|| anyhow::anyhow!("没有可发送的用户消息"))?;

        println!("📝 输入内容: {:?}", input);
        println!("📝 输入长度: {} 字符", input.len());

        // 检查 input 是否为空（双重保险）
        if input.trim().is_empty() {
            return Err(anyhow::anyhow!("输入内容为空"));
        }

        // 构建请求体 - 使用 Responses API 格式
        let request_body = json!({
            "model": &self.config.model,
            "input": input.trim(),
            "reasoning": {
                "effort": self.config.reasoning_effort,
                "summary": self.config.reasoning_summary,
            },
            "stream": true,
        });

        // 构建 API base URL - 使用 Responses API 端点
        let api_base = self.config.api_base.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/responses", api_base);

        println!("🔗 使用 Responses API 调用: {}", url);
        println!("📝 模型: {}", &self.config.model);
        println!("🧠 推理模式: summary=auto");
        println!("📦 请求体: {}", serde_json::to_string_pretty(&request_body).unwrap_or_default());

        // 创建 HTTP 客户端
        let client = reqwest::Client::new();

        // 发送请求
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        // 检查响应状态
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("API 请求失败 ({}): {}", status, error_text));
        }

        let mut full_response = String::new();
        let mut reasoning_summary = String::new();
        let mut stream = response.bytes_stream();

        // 解析 SSE 流 - Responses API 格式
        let mut buffer = Vec::new();
        let mut current_event = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            buffer.extend_from_slice(&chunk);

            // 处理缓冲区中的所有完整行
            while let Some(line_end) = buffer.iter().position(|&b| b == b'\n') {
                let line = buffer[..line_end].to_vec();
                buffer.drain(..=line_end);

                let line_str = String::from_utf8_lossy(&line);
                let line_str = line_str.trim();

                // 跳过空行和注释
                if line_str.is_empty() || line_str.starts_with(':') {
                    continue;
                }

                // Responses API SSE 格式：先是 event: 行，然后是 data: 行
                if let Some(event_type) = line_str.strip_prefix("event: ") {
                    current_event = event_type.to_string();
                    continue;
                }

                // 检查是否是 data: 行
                if let Some(data) = line_str.strip_prefix("data: ") {
                    // [DONE] 表示流结束
                    if data == "[DONE]" {
                        println!("✅ 流式传输完成");
                        break;
                    }

                    // 解析 JSON
                    match serde_json::from_str::<serde_json::Value>(data) {
                        Ok(json) => {
                            // 根据事件类型处理数据
                            match current_event.as_str() {
                                "response.reasoning_summary_text.delta" => {
                                    // 提取推理摘要增量
                                    if let Some(delta) = json["delta"].as_str() {
                                        reasoning_summary.push_str(delta);
                                        on_reasoning(delta.to_string());
                                        println!("🧠 推理摘要delta: {} 字符", delta.len());
                                    }
                                }
                                "response.output_text.delta" => {
                                    // 提取输出文本增量
                                    if let Some(delta) = json["delta"].as_str() {
                                        full_response.push_str(delta);
                                        on_chunk(delta.to_string());
                                        println!("📦 输出delta: {} 字符", delta.len());
                                    }
                                }
                                "response.completed" => {
                                    println!("✅ 响应完成事件");
                                }
                                _ => {
                                    // 其他事件类型，暂时忽略
                                    if !current_event.is_empty() {
                                        println!("ℹ️ 其他事件: {}", current_event);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("⚠️ 解析 JSON 失败: {} - 事件: {} - 数据: {}", e, current_event, data);
                        }
                    }
                }
            }
        }

        // 返回结果
        let reasoning_opt = if reasoning_summary.is_empty() {
            None
        } else {
            Some(reasoning_summary)
        };

        println!("✅ Responses API 完成 - 输出: {} 字符, 推理摘要: {:?}",
                 full_response.len(),
                 reasoning_opt.as_ref().map(|s| format!("{} 字符", s.len())));

        Ok((full_response, reasoning_opt))
    }

    // 无自动回退：是否使用 Responses API 由用户配置决定

    /// 将消息转换为 JSON 格式（用于原生 HTTP 客户端）
    fn convert_messages_to_json(&self, messages: &[Message]) -> Result<Vec<serde_json::Value>> {
        let mut json_messages = Vec::new();

        // 添加系统消息
        json_messages.push(json!({
            "role": "system",
            "content": "You are a helpful AI assistant. You can use Markdown formatting in your responses."
        }));

        // 添加对话历史
        for msg in messages {
            json_messages.push(json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        Ok(json_messages)
    }

    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<ChatCompletionRequestMessage>> {
        let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();

        // Add system message
        openai_messages.push(
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You are a helpful AI assistant. You can use Markdown formatting in your responses.")
                .build()?
                .into(),
        );

        // Add conversation history
        for msg in messages {
            if msg.is_user() {
                openai_messages.push(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?
                        .into(),
                );
            } else {
                // For assistant messages, we need to use a different type
                openai_messages.push(
                    ChatCompletionRequestMessage::Assistant(
                        async_openai::types::ChatCompletionRequestAssistantMessageArgs::default()
                            .content(msg.content.clone())
                            .build()?
                    )
                );
            }
        }

        Ok(openai_messages)
    }
}

impl Default for OpenAIService {
    fn default() -> Self {
        Self::new()
    }
}
