use anyhow::Result;
use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
};
use futures::StreamExt;
use reqwest::{StatusCode, header::HeaderMap};
use serde_json::json;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};

use super::events::{EventParser, RateLimitSnapshot, ResponseEvent};
use crate::models::Message;
use crate::tools::spec::ToolInvocation;
use crate::tools::{ApprovalPolicy, RuntimeConfig, ToolRegistry, ToolRouter, ToolRuntime};

const DEFAULT_SYSTEM_PROMPT: &str = "You are a helpful AI assistant. Always respond with well-structured Markdown that uses descriptive headings (#, ##, ###), bullet or numbered lists, bold and italic emphasis, tables when helpful, and fenced code blocks for code examples. Maintain the user's language unless instructed otherwise.";

/// Configuration for OpenAI API
#[derive(Clone, Debug)]
pub struct OpenAIServiceConfig {
    pub api_key: String,
    pub api_base: Option<String>,
    pub model: String,
    pub use_responses_api: bool, // 是否使用 Responses API（第三方服务可能不支持）
    // Responses API 推理选项
    pub reasoning_effort: String, // minimal|low|medium|high（默认：medium）
    pub reasoning_summary: String, // auto|on|off（默认：auto）
    // 工具配置
    pub enable_tools: bool, // 是否启用工具调用（默认：false）
    pub tool_approval_policy: ApprovalPolicy, // 工具审批策略
    // 可靠性配置
    pub retry_policy: RetryPolicyConfig,
    pub timeout_config: TimeoutConfig,
    pub telemetry_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct RetryPolicyConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_ms: u64,
}

impl RetryPolicyConfig {
    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let capped_attempt = attempt.saturating_sub(1).min(10);
        let mut delay = (self.base_delay_ms as u128) * (1_u128 << capped_attempt);
        delay = delay.min(self.max_delay_ms as u128);
        if self.jitter_ms > 0 {
            let jitter = (attempt as u128 * 37) % self.jitter_ms as u128;
            delay = delay.saturating_add(jitter).min(self.max_delay_ms as u128);
        }
        Duration::from_millis(delay as u64)
    }
}

#[derive(Clone, Debug)]
pub struct TimeoutConfig {
    pub request_timeout: Duration,
    pub stream_idle_timeout: Duration,
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

        // 工具配置
        let enable_tools = std::env::var("OPENAI_ENABLE_TOOLS")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false);

        let tool_approval_policy = match std::env::var("OPENAI_TOOL_APPROVAL").ok().as_deref() {
            Some("auto") => ApprovalPolicy::AutoApprove,
            Some("safe") => ApprovalPolicy::AutoApproveSafe,
            Some("require") => ApprovalPolicy::RequireApproval,
            _ => ApprovalPolicy::AutoApproveSafe, // 默认：安全工具自动批准
        };

        let retry_policy = RetryPolicyConfig {
            max_attempts: read_env_u32("OPENAI_RETRY_MAX_ATTEMPTS", 3),
            base_delay_ms: read_env_u64("OPENAI_RETRY_BASE_DELAY_MS", 500),
            max_delay_ms: read_env_u64("OPENAI_RETRY_MAX_DELAY_MS", 8_000),
            jitter_ms: read_env_u64("OPENAI_RETRY_JITTER_MS", 250),
        };

        let timeout_config = TimeoutConfig {
            request_timeout: Duration::from_millis(read_env_u64(
                "OPENAI_REQUEST_TIMEOUT_MS",
                15_000,
            )),
            stream_idle_timeout: Duration::from_millis(read_env_u64(
                "OPENAI_STREAM_IDLE_TIMEOUT_MS",
                20_000,
            )),
        };

        Ok(Self {
            api_key,
            api_base,
            model,
            use_responses_api,
            reasoning_effort: std::env::var("OPENAI_REASONING_EFFORT")
                .unwrap_or_else(|_| "medium".to_string()),
            reasoning_summary: std::env::var("OPENAI_REASONING_SUMMARY")
                .unwrap_or_else(|_| "auto".to_string()),
            enable_tools,
            tool_approval_policy,
            retry_policy,
            timeout_config,
            telemetry_enabled: read_env_bool("OPENAI_TELEMETRY_ENABLED", true),
        })
    }

    /// Create config with custom values
    #[allow(dead_code)]
    pub fn new(api_key: String, api_base: Option<String>, model: String) -> Self {
        Self {
            api_key,
            api_base,
            model,
            use_responses_api: true, // 默认使用 Responses API
            reasoning_effort: "medium".to_string(),
            reasoning_summary: "auto".to_string(),
            enable_tools: false,
            tool_approval_policy: ApprovalPolicy::AutoApproveSafe,
            retry_policy: RetryPolicyConfig {
                max_attempts: 3,
                base_delay_ms: 500,
                max_delay_ms: 8_000,
                jitter_ms: 250,
            },
            timeout_config: TimeoutConfig {
                request_timeout: Duration::from_millis(15_000),
                stream_idle_timeout: Duration::from_millis(20_000),
            },
            telemetry_enabled: true,
        }
    }
}

#[derive(Clone)]
pub struct OpenAIService {
    client: Client<OpenAIConfig>,
    config: OpenAIServiceConfig,
    tool_registry: Arc<ToolRegistry>,
    tool_runtime: Arc<ToolRuntime>,
    mcp_enabled: bool,
    mcp_manager: Option<Arc<tokio::sync::Mutex<crate::mcp::McpConnectionManager>>>,
    reqwest_client: reqwest::Client,
    rate_limit_tracker: Arc<RateLimitTracker>,
    telemetry: Arc<TelemetryState>,
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
                enable_tools: false,
                tool_approval_policy: ApprovalPolicy::AutoApproveSafe,
                retry_policy: RetryPolicyConfig {
                    max_attempts: 3,
                    base_delay_ms: 500,
                    max_delay_ms: 8_000,
                    jitter_ms: 250,
                },
                timeout_config: TimeoutConfig {
                    request_timeout: Duration::from_millis(15_000),
                    stream_idle_timeout: Duration::from_millis(20_000),
                },
                telemetry_enabled: true,
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

        // 初始化工具系统
        let mut tool_registry = ToolRegistry::new();
        crate::tools::builtin::register_builtin_tools(&mut tool_registry);

        let tool_runtime_config = RuntimeConfig {
            approval_policy: config.tool_approval_policy.clone(),
            timeout_ms: 60000,
            sandboxed: false,
        };

        let tool_registry = Arc::new(tool_registry);
        let tool_runtime = Arc::new(ToolRuntime::new(tool_registry.clone(), tool_runtime_config));
        let reqwest_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");
        let telemetry_enabled = config.telemetry_enabled;

        Self {
            client,
            config,
            tool_registry,
            tool_runtime,
            mcp_enabled: false,
            mcp_manager: None,
            reqwest_client,
            rate_limit_tracker: Arc::new(RateLimitTracker::new()),
            telemetry: Arc::new(TelemetryState::new(telemetry_enabled)),
        }
    }

    /// 初始化 MCP 集成
    ///
    /// 加载 MCP 配置，连接服务器，并注册工具
    /// 这是一个异步方法，应该在服务创建后调用
    ///
    /// 环境变量:
    /// - ENABLE_MCP: "true" 启用 MCP 集成（默认: true）
    /// - MCP_SERVERS_CONFIG: 配置文件路径（可选）
    pub async fn initialize_mcp(&mut self) -> Result<usize> {
        use crate::mcp::{McpConfig, McpConnectionManager};

        // 检查是否启用 MCP
        let enable_mcp = std::env::var("ENABLE_MCP")
            .ok()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(true); // 默认启用

        if !enable_mcp {
            println!("ℹ️  MCP 集成已禁用（ENABLE_MCP=false）");
            return Ok(0);
        }

        println!("🔌 初始化 MCP 集成...");

        // 1. 加载配置
        let mcp_config = match McpConfig::load_default()? {
            Some(config) => {
                println!("✅ 加载到 {} 个 MCP 服务器配置", config.server_count());
                config
            }
            None => {
                println!("ℹ️  未找到 MCP 配置，跳过 MCP 集成");
                return Ok(0);
            }
        };

        if mcp_config.enabled_count() == 0 {
            println!("ℹ️  没有启用的 MCP 服务器");
            return Ok(0);
        }

        // 2. 创建连接管理器
        let mut manager = McpConnectionManager::from_configs(mcp_config.servers.clone());

        // 3. 连接到所有服务器
        manager.connect_all().await?;

        if manager.connection_count() == 0 {
            println!("⚠️  未能连接到任何 MCP 服务器");
            return Ok(0);
        }

        // 4. 发现并注册工具
        // 需要获取可变引用到 tool_registry
        // 由于 tool_registry 是 Arc，我们需要使用 Arc::get_mut 或者使用内部可变性
        // 这里我们使用一个临时的可变 registry 然后重新创建 Arc

        // 创建一个新的 registry，包含现有工具
        let mut new_registry = ToolRegistry::new();

        // 重新注册内置工具
        crate::tools::builtin::register_builtin_tools(&mut new_registry);

        // 注册 MCP 工具
        let tool_count = match manager.discover_and_register_tools(&mut new_registry).await {
            Ok(_) => {
                let count = new_registry.specs().len();
                println!("🎉 MCP 集成完成！总共 {} 个工具可用", count);
                count
            }
            Err(e) => {
                println!("⚠️  MCP 工具发现失败: {}", e);
                0
            }
        };

        // 更新 tool_registry
        self.tool_registry = Arc::new(new_registry);

        // 更新 tool_runtime 使用新的 registry
        let tool_runtime_config = RuntimeConfig {
            approval_policy: self.config.tool_approval_policy.clone(),
            timeout_ms: 60000,
            sandboxed: false,
        };
        self.tool_runtime = Arc::new(ToolRuntime::new(
            self.tool_registry.clone(),
            tool_runtime_config,
        ));

        // 保存 manager 用于后续管理
        self.mcp_manager = Some(Arc::new(tokio::sync::Mutex::new(manager)));
        self.mcp_enabled = true;

        Ok(tool_count)
    }

    /// 检查是否已启用 MCP
    pub fn is_mcp_enabled(&self) -> bool {
        self.mcp_enabled
    }

    /// 获取 MCP 连接状态
    pub async fn get_mcp_status(
        &self,
    ) -> Result<std::collections::HashMap<String, crate::mcp::ConnectionStatus>> {
        if let Some(manager) = &self.mcp_manager {
            let mgr = manager.lock().await;
            Ok(mgr.get_status().await)
        } else {
            Ok(std::collections::HashMap::new())
        }
    }

    /// 重新连接指定的 MCP 服务器
    pub async fn reconnect_mcp_server(&mut self, server_name: &str) -> Result<()> {
        if let Some(manager) = &self.mcp_manager {
            let mut mgr = manager.lock().await;

            // 找到对应的配置
            let config = mgr
                .configs
                .iter()
                .find(|c| c.name == server_name)
                .ok_or_else(|| anyhow::anyhow!("未找到服务器配置: {}", server_name))?
                .clone();

            println!("🔄 重新连接 MCP 服务器: {}", server_name);

            // 先断开旧连接（如果存在）
            if let Some(old_conn) = mgr.connections.remove(server_name) {
                let mut conn = old_conn.lock().await;
                let _ = conn.disconnect().await;
            }

            // 创建新连接
            let connection_arc = mgr.create_connection(&config)?;
            {
                let mut conn = connection_arc.lock().await;
                conn.connect().await?;
            }

            mgr.connections
                .insert(server_name.to_string(), connection_arc);
            println!("✅ MCP 服务器 {} 重连成功", server_name);

            Ok(())
        } else {
            Err(anyhow::anyhow!("MCP 未启用"))
        }
    }

    /// 对所有 MCP 连接进行健康检查
    pub async fn health_check_mcp(&self) -> Result<Vec<(String, bool)>> {
        if let Some(manager) = &self.mcp_manager {
            let mut results = Vec::new();
            let mgr = manager.lock().await;

            for (name, connection) in &mgr.connections {
                let mut conn = connection.lock().await;
                let healthy = conn.health_check().await.unwrap_or(false);
                results.push((name.clone(), healthy));

                if healthy {
                    println!("✅ MCP 服务器 {} 健康", name);
                } else {
                    println!("❌ MCP 服务器 {} 不健康", name);
                }
            }

            Ok(results)
        } else {
            Ok(Vec::new())
        }
    }

    /// 断开所有 MCP 连接
    pub async fn disconnect_all_mcp(&mut self) -> Result<()> {
        if let Some(manager) = &self.mcp_manager {
            let mut mgr = manager.lock().await;
            mgr.disconnect_all().await?;
            println!("✅ 所有 MCP 连接已断开");
        }
        Ok(())
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
    pub async fn get_streaming_completion<F>(
        &self,
        messages: &[Message],
        mut on_chunk: F,
    ) -> Result<(String, Option<String>)>
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

    /// 使用原生 HTTP 客户端的流式完成（支持工具调用）
    ///
    /// 根据配置自动选择：
    /// - Responses API (支持 reasoning summary)
    /// - Chat Completions API (传统API，更兼容)
    ///
    /// # 返回
    /// - `CompletionResult`: 包含内容、推理摘要和工具调用
    pub async fn get_streaming_completion_native<F1, F2>(
        &self,
        messages: &[Message],
        on_chunk: F1,
        on_reasoning: F2,
    ) -> Result<super::CompletionResult>
    where
        F1: FnMut(String) + Send,
        F2: FnMut(String) + Send,
    {
        // 为了在需要时回退到 Chat Completions，可以对同一闭包多次调用，改为在内部以 &mut 方式转发
        let mut on_chunk_mut = on_chunk;
        let mut on_reasoning_mut = on_reasoning;

        if self.config.use_responses_api {
            println!("ℹ️ 使用 Responses API（支持 reasoning summary + 工具调用）");
            self.call_responses_api(messages, &mut on_chunk_mut, &mut on_reasoning_mut)
                .await
        } else {
            println!("ℹ️ 使用 Chat Completions API（传统模式）");
            let (content, reasoning) = self
                .call_chat_completions_api(messages, &mut on_chunk_mut)
                .await?;
            Ok(super::CompletionResult::simple(content, reasoning))
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

        let api_messages = self.convert_messages_to_json(messages)?;
        let request_body = json!({
            "model": &self.config.model,
            "messages": api_messages,
            "stream": true,
        });

        let api_base = self
            .config
            .api_base
            .as_deref()
            .unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/chat/completions", api_base);

        println!("🔗 调用 Chat Completions API: {}", url);
        println!("📝 模型: {}", &self.config.model);

        let payload = Arc::new(request_body);
        let url_arc = Arc::new(url);
        let response = self
            .execute_with_retry("chat_request", || {
                let payload = Arc::clone(&payload);
                let url = Arc::clone(&url_arc);
                async move { self.send_authenticated_post(&url, payload).await }
            })
            .await?;

        self.rate_limit_tracker
            .update_from_headers(response.headers());

        let mut full_response = String::new();
        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();

        while let Some(chunk_result) = match timeout(
            self.config.timeout_config.stream_idle_timeout,
            stream.next(),
        )
        .await
        {
            Ok(item) => item,
            Err(_) => {
                return Err(anyhow::anyhow!(
                    "Chat Completions 流在 {:?} 内无数据，已超时",
                    self.config.timeout_config.stream_idle_timeout
                ));
            }
        } {
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

        println!(
            "✅ Chat Completions API 完成 - 输出: {} 字符",
            full_response.len()
        );
        Ok((full_response, None))
    }

    /// 使用 Responses API    /// 使用 Responses API（支持工具调用）
    async fn call_responses_api<F1, F2>(
        &self,
        messages: &[Message],
        on_chunk: &mut F1,
        on_reasoning: &mut F2,
    ) -> Result<super::CompletionResult>
    where
        F1: FnMut(String) + Send,
        F2: FnMut(String) + Send,
    {
        use futures::StreamExt;

        self.telemetry.record_request();
        let overall_start = Instant::now();

        let result = async {
            let input = messages
                .iter()
                .rev()
                .find(|m| m.is_user() && !m.content.trim().is_empty())
                .map(|m| m.content.clone())
                .ok_or_else(|| anyhow::anyhow!("没有可发送的用户消息"))?;

            if input.trim().is_empty() {
                return Err(anyhow::anyhow!("输入内容为空"));
            }
            let instructions = self.build_responses_instructions(messages)?;

            let mut request_body = json!({
                "model": &self.config.model,
                "input": input.trim(),
                "instructions": instructions,
                "reasoning": {
                    "effort": self.config.reasoning_effort,
                    "summary": self.config.reasoning_summary,
                },
                "stream": true,
            });

            if self.config.enable_tools {
                let tool_specs = self.tool_registry.specs();
                if !tool_specs.is_empty() {
                    let tools_json: Vec<serde_json::Value> =
                        tool_specs.iter().map(|spec| spec.to_json()).collect();

                    request_body["tools"] = json!(tools_json);
                    request_body["tool_choice"] = json!("auto");
                    request_body["parallel_tool_calls"] = json!(true);

                    println!("🔧 启用工具调用: {} 个工具", tools_json.len());
                    for spec in &tool_specs {
                        println!("   • {}", spec.name);
                    }
                } else {
                    println!("ℹ️ 启用了工具调用，但当前注册表为空");
                }
            }

            let api_base = self
                .config
                .api_base
                .as_deref()
                .unwrap_or("https://api.openai.com/v1");
            let url = format!("{}/responses", api_base);

            println!("🔗 使用 Responses API 调用: {}", url);
            println!("📝 模型: {}", &self.config.model);

            let payload = Arc::new(request_body);
            let url_arc = Arc::new(url);

            let response = self
                .execute_with_retry("responses_request", || {
                    let payload = Arc::clone(&payload);
                    let url = Arc::clone(&url_arc);
                    async move { self.send_authenticated_post(&url, payload).await }
                })
                .await?;

            self.rate_limit_tracker
                .update_from_headers(response.headers());

            let mut full_response = String::new();
            let mut reasoning_summary = String::new();
            let mut reasoning_content = String::new();
            let mut stream = response.bytes_stream();
            let mut buffer = Vec::new();
            let mut tool_invocations: Vec<ToolInvocation> = Vec::new();
            let mut event_parser = EventParser::new();

            while let Some(chunk_result) = match timeout(
                self.config.timeout_config.stream_idle_timeout,
                stream.next(),
            )
            .await
            {
                Ok(item) => item,
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Responses 流在 {:?} 内无数据，已超时",
                        self.config.timeout_config.stream_idle_timeout
                    ));
                }
            } {
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

                    if let Some(event_type) = line_str.strip_prefix("event: ") {
                        event_parser.set_event_type(event_type.to_string());
                        continue;
                    }

                    if let Some(data) = line_str.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            println!("✅ 流式传输完成");
                            break;
                        }

                        match event_parser.parse_data(data) {
                            Ok(Some(event)) => match event {
                                ResponseEvent::Created { response_id } => {
                                    println!("🚀 响应开始 | ID: {}", response_id);
                                }
                                ResponseEvent::OutputTextDelta(delta) => {
                                    full_response.push_str(&delta);
                                    on_chunk(delta.clone());
                                }
                                ResponseEvent::ReasoningSummaryDelta(delta) => {
                                    reasoning_summary.push_str(&delta);
                                    on_reasoning(delta.clone());
                                }
                                ResponseEvent::ReasoningContentDelta(delta) => {
                                    reasoning_content.push_str(&delta);
                                }
                                ResponseEvent::ReasoningSummaryPartAdded => {}
                                ResponseEvent::OutputItemDone(item) => {
                                    if let Ok(Some(invocation)) =
                                        ToolRouter::build_tool_invocation(item)
                                    {
                                        println!(
                                            "🔧 检测到工具调用: {} (call_id: {})",
                                            invocation.name, invocation.call_id
                                        );
                                        tool_invocations.push(invocation);
                                    }
                                }
                                ResponseEvent::OutputItemAdded(_) => {}
                                ResponseEvent::Completed { .. } => {}
                                ResponseEvent::RateLimits(snapshot) => {
                                    println!(
                                        "🚦 速率限制: requests={}/{}, tokens={}/{}",
                                        snapshot.requests.remaining,
                                        snapshot.requests.limit,
                                        snapshot.tokens.remaining,
                                        snapshot.tokens.limit
                                    );
                                    self.rate_limit_tracker.record_snapshot(&snapshot);
                                }
                                ResponseEvent::Failed { error, retry_after } => {
                                    if let Some(secs) = retry_after {
                                        self.rate_limit_tracker
                                            .record_retry_after(Duration::from_secs(secs));
                                    }
                                    return Err(anyhow::anyhow!("Response failed: {}", error));
                                }
                            },
                            Ok(None) => {}
                            Err(e) => {
                                println!("⚠️ 事件解析失败: {} | 数据: {}", e, data);
                            }
                        }
                    }
                }
            }

            if !event_parser.saw_completed() {
                println!("⚠️ 警告: 流关闭但未收到 response.completed 事件");
            }

            let reasoning_opt = if reasoning_summary.is_empty() {
                None
            } else {
                Some(reasoning_summary)
            };

            let completion =
                super::CompletionResult::new(full_response, reasoning_opt, tool_invocations);

            println!(
                "✅ Responses API 完成 - 输出: {} 字符, 推理摘要: {:?}, 工具调用: {}",
                completion.content.len(),
                completion
                    .reasoning_summary
                    .as_ref()
                    .map(|s| format!("{} 字符", s.len())),
                completion.tool_calls.len()
            );
            println!("   🔬 推理文本累计: {} 字符", reasoning_content.len());

            Ok(completion)
        }
        .await;

        match &result {
            Ok(_) => self.telemetry.record_success(overall_start.elapsed()),
            Err(_) => self.telemetry.record_failure(overall_start.elapsed()),
        }

        result
    }

    /// 将消息转换为 JSON 格式（用于原生 HTTP 客户端）
    fn convert_messages_to_json(&self, messages: &[Message]) -> Result<Vec<serde_json::Value>> {
        let mut json_messages = Vec::new();

        // 添加系统消息 - 明确要求Markdown格式
        json_messages.push(json!({
            "role": "system",
            "content": DEFAULT_SYSTEM_PROMPT
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

    /// 构建 Responses API instructions：包括系统提示和对话历史
    fn build_responses_instructions(&self, messages: &[Message]) -> Result<String> {
        let mut instructions = String::from(DEFAULT_SYSTEM_PROMPT);

        if !messages.is_empty() {
            instructions.push_str("\n\nConversation history (latest last):");
            for msg in messages {
                if msg.content.trim().is_empty() {
                    continue;
                }
                if msg.is_user() {
                    instructions.push_str("\n[User] ");
                } else {
                    instructions.push_str("\n[Assistant] ");
                }
                instructions.push_str(msg.content.trim());
            }
        }

        Ok(instructions)
    }

    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<ChatCompletionRequestMessage>> {
        let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();

        // Add system message - 明确要求Markdown格式
        openai_messages.push(
            ChatCompletionRequestSystemMessageArgs::default()
                .content(DEFAULT_SYSTEM_PROMPT)
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
                openai_messages.push(ChatCompletionRequestMessage::Assistant(
                    async_openai::types::ChatCompletionRequestAssistantMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?,
                ));
            }
        }

        Ok(openai_messages)
    }

    async fn execute_with_retry<T, F, Fut>(&self, label: &str, mut operation: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, RetryableError>>,
    {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match operation().await {
                Ok(value) => return Ok(value),
                Err(err) => {
                    if !err.retryable || attempt >= self.config.retry_policy.max_attempts {
                        return Err(anyhow::Error::new(err));
                    }

                    self.telemetry.record_retry();
                    let delay = err
                        .retry_after
                        .unwrap_or_else(|| self.config.retry_policy.delay_for_attempt(attempt));
                    println!(
                        "🔁 [{}] 第{}次尝试失败: {}。将在 {:?} 后重试",
                        label, attempt, err, delay
                    );
                    sleep(delay).await;
                }
            }
        }
    }

    async fn send_authenticated_post(
        &self,
        url: &str,
        body: Arc<serde_json::Value>,
    ) -> Result<reqwest::Response, RetryableError> {
        self.rate_limit_tracker.wait_before_request().await;
        let send_future = self
            .reqwest_client
            .post(url)
            .header("Authorization", format!("Bearer {}", &self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&*body)
            .send();

        let response = match timeout(self.config.timeout_config.request_timeout, send_future).await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(err)) => {
                if err.is_timeout() || err.is_connect() || err.is_request() {
                    return Err(RetryableError::retryable(
                        format!("网络错误: {}", err),
                        None,
                    ));
                }
                return Err(RetryableError::fatal(format!("请求失败: {}", err)));
            }
            Err(_) => {
                return Err(RetryableError::retryable(
                    format!("请求超时 {:?}", self.config.timeout_config.request_timeout),
                    Some(self.config.timeout_config.request_timeout),
                ));
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let retry_after = parse_retry_after(response.headers());
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<无法读取错误响应>".to_string());

            if should_retry_status(status) {
                return Err(RetryableError::retryable(
                    format!("API 请求失败 ({}): {}", status, error_text),
                    retry_after,
                ));
            } else {
                return Err(RetryableError::fatal(format!(
                    "API 请求失败 ({}): {}",
                    status, error_text
                )));
            }
        }

        Ok(response)
    }
}

impl Default for OpenAIService {
    fn default() -> Self {
        Self::new()
    }
}

/// 重试错误类型，用于传递 retry_after 等元数据
#[derive(Debug)]
struct RetryableError {
    message: String,
    retry_after: Option<Duration>,
    retryable: bool,
}

impl RetryableError {
    fn retryable(message: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self {
            message: message.into(),
            retry_after,
            retryable: true,
        }
    }

    fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after: None,
            retryable: false,
        }
    }
}

impl std::fmt::Display for RetryableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RetryableError {}

struct RateLimitTracker {
    next_allowed: Mutex<Option<Instant>>,
}

impl RateLimitTracker {
    fn new() -> Self {
        Self {
            next_allowed: Mutex::new(None),
        }
    }

    async fn wait_before_request(&self) {
        if let Some(deadline) = *self.next_allowed.lock().unwrap() {
            let now = Instant::now();
            if deadline > now {
                let delay = deadline - now;
                println!("⏳ 等待速率限制 {:?}", delay);
                sleep(delay).await;
            }
        }
    }

    fn record_retry_after(&self, wait: Duration) {
        let mut guard = self.next_allowed.lock().unwrap();
        let target = Instant::now() + wait;
        if guard.map_or(true, |current| target > current) {
            *guard = Some(target);
        }
    }

    fn record_snapshot(&self, snapshot: &RateLimitSnapshot) {
        let now = std::time::SystemTime::now();
        if snapshot.requests.remaining == 0 {
            if let Ok(wait) = snapshot.requests.reset_at.duration_since(now) {
                self.record_retry_after(wait);
            }
        }
        if snapshot.tokens.remaining == 0 {
            if let Ok(wait) = snapshot.tokens.reset_at.duration_since(now) {
                self.record_retry_after(wait);
            }
        }
    }

    fn update_from_headers(&self, headers: &HeaderMap) {
        if let Some(wait) = parse_rate_limit_from_headers(headers) {
            self.record_retry_after(wait);
        }
    }
}

struct TelemetryState {
    enabled: bool,
    requests: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    retries: AtomicU64,
    total_latency_ms: AtomicU64,
}

impl TelemetryState {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            requests: AtomicU64::new(0),
            successes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            retries: AtomicU64::new(0),
            total_latency_ms: AtomicU64::new(0),
        }
    }

    fn record_request(&self) {
        if self.enabled {
            self.requests.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn record_success(&self, duration: Duration) {
        if self.enabled {
            self.successes.fetch_add(1, Ordering::SeqCst);
            self.total_latency_ms
                .fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
        }
    }

    fn record_failure(&self, duration: Duration) {
        if self.enabled {
            self.failures.fetch_add(1, Ordering::SeqCst);
            self.total_latency_ms
                .fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
        }
    }

    fn record_retry(&self) {
        if self.enabled {
            self.retries.fetch_add(1, Ordering::SeqCst);
        }
    }
}

fn read_env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(default)
}

fn read_env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(default)
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64)
}

fn parse_rate_limit_from_headers(headers: &HeaderMap) -> Option<Duration> {
    if let Some(remaining) = header_u64(headers, "x-ratelimit-remaining-requests") {
        if remaining == 0 {
            if let Some(reset) = header_f64(headers, "x-ratelimit-reset-requests") {
                return Some(Duration::from_secs_f64(reset));
            }
        }
    }
    if let Some(remaining) = header_u64(headers, "x-ratelimit-remaining-tokens") {
        if remaining == 0 {
            if let Some(reset) = header_f64(headers, "x-ratelimit-reset-tokens") {
                return Some(Duration::from_secs_f64(reset));
            }
        }
    }
    None
}

fn header_u64(headers: &HeaderMap, key: &str) -> Option<u64> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

fn header_f64(headers: &HeaderMap, key: &str) -> Option<f64> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok())
}

fn should_retry_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}
