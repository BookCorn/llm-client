# 架构分析：当前实现 vs Codex-CLI 最佳实践

> **分析时间**: 2025-11-07
> **目标**: 对比当前 GPUI Chat 应用与 Codex-CLI 的 Responses API 实现，识别改进机会

---

## 📊 执行摘要

当前实现已成功完成 **Responses API 的基础集成**，包括流式输出、推理摘要显示、UI性能优化等核心功能。然而，对比 Codex-CLI 的生产级实现，存在以下关键差距：

| 功能领域 | 当前状态 | Codex-CLI 标准 | 优先级 |
|---------|----------|----------------|--------|
| SSE 事件处理 | 部分实现（2/10事件） | 完整事件映射 | 🔴 高 |
| 工具调用 | ❌ 未实现 | 完整工具编排 | 🔴 高 |
| MCP 集成 | ❌ 未实现 | 完整 MCP 客户端 | 🔴 高 |
| 错误处理 | 基础错误显示 | 重试+退避+超时 | 🟡 中 |
| 数据模型 | 简单 Message | 结构化 ResponseItem | 🟡 中 |
| 速率限制 | ❌ 未处理 | RateLimitSnapshot | 🟢 低 |
| 遥测 | ❌ 无 | OtelEventManager | 🟢 低 |

---

## 1️⃣ 事件驱动架构

### 当前实现

```rust
// src/services/openai.rs:471-502
match current_event.as_str() {
    "response.reasoning_summary_text.delta" => {
        if let Some(delta) = json["delta"].as_str() {
            reasoning_summary.push_str(delta);
            on_reasoning(delta.to_string());
        }
    }
    "response.output_text.delta" => {
        if let Some(delta) = json["delta"].as_str() {
            full_response.push_str(delta);
            on_chunk(delta.to_string());
        }
    }
    _ => {}
}
```

**问题**:
- ❌ 只处理 2 种事件（delta事件），忽略其他 8+ 种关键事件
- ❌ 无法处理 `response.output_item.done`（工具调用的核心事件）
- ❌ 直接字符串匹配，缺少类型安全
- ❌ 无事件抽象层，难以扩展

### Codex-CLI 实现

```rust
// codex-rs/core/src/client_common.rs:197
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    OutputItemAdded(ResponseItem),
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),      // Raw reasoning
    ReasoningSummaryPartAdded,          // Section breaks
    Completed { response_id: String, token_usage: TokenUsage },
    RateLimits(RateLimitSnapshot),
    Failed { error: String, retry_after: Option<u64> },
}

// codex-rs/core/src/client.rs:792
"response.output_item.done" => {
    let item = parse_response_item(json)?;
    emit(ResponseEvent::OutputItemDone(item));  // 边产出边处理！
}
```

**优势**:
- ✅ 完整的事件枚举，类型安全
- ✅ 支持工具调用（通过 OutputItemDone）
- ✅ 支持详细推理（reasoning_text.delta）
- ✅ 错误事件包含重试建议
- ✅ "边产出边处理"模式（即时消费工具调用）

### 改进建议

**阶段1: 创建事件抽象层**
```rust
// src/services/events.rs (新文件)
pub enum ResponseEvent {
    Created,
    OutputItemDone(ResponseItem),
    OutputTextDelta(String),
    ReasoningSummaryDelta(String),
    ReasoningContentDelta(String),
    ReasoningSummaryPartAdded,
    Completed { response_id: String },
    RateLimits(RateLimitInfo),
    Failed { error: String },
}

pub enum ResponseItem {
    Message { role: String, content: String },
    Reasoning { summary: Vec<String>, content: Option<String> },
    FunctionCall { call_id: String, name: String, arguments: String },
    FunctionCallOutput { call_id: String, output: String },
}
```

**阶段2: 重构SSE解析器**
```rust
// 将当前的字符串匹配替换为事件映射
fn parse_sse_event(event_type: &str, data: Value) -> Result<ResponseEvent> {
    match event_type {
        "response.created" => Ok(ResponseEvent::Created),
        "response.output_item.done" => {
            let item = parse_response_item(data)?;
            Ok(ResponseEvent::OutputItemDone(item))
        }
        "response.output_text.delta" => {
            Ok(ResponseEvent::OutputTextDelta(data["delta"].as_str()?.to_string()))
        }
        // ... 完整映射
    }
}
```

---

## 2️⃣ 工具调用系统

### 当前实现

**状态**: ❌ 完全缺失

当前代码：
- 无工具定义机制
- 无工具路由器
- 无工具执行运行时
- 请求中不包含 `tools` 字段

### Codex-CLI 实现

**核心组件**:

1. **ToolSpec 定义** (codex-rs/core/src/tools/spec.rs)
```rust
pub fn create_tools_json_for_responses_api() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "name": "shell",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                },
                "required": ["command"]
            }
        }),
        // ... 更多工具
    ]
}
```

2. **ToolRouter 路由** (codex-rs/core/src/tools/router.rs:37)
```rust
pub fn build_tool_call(item: ResponseItem) -> ToolInvocation {
    match item {
        ResponseItem::FunctionCall { call_id, name, arguments } => {
            let args = serde_json::from_str(&arguments)?;
            ToolInvocation { call_id, name, args }
        }
        // ... 其他工具类型
    }
}
```

3. **ToolCallRuntime 执行** (codex-rs/core/src/tools/runtimes/*)
```rust
pub async fn handle_tool_call(invocation: ToolInvocation) -> ResponseInputItem {
    // 执行工具（支持审批、沙箱、并行）
    let output = execute_tool(invocation.name, invocation.args).await?;

    ResponseInputItem::FunctionCallOutput {
        call_id: invocation.call_id,
        output: serialize_output(output),
    }
}
```

4. **工具回传与下一轮** (codex-rs/core/src/response_processing.rs)
```rust
// 将工具调用+输出写入历史，构建下一轮 input
fn process_items(items: Vec<ResponseItem>) -> Vec<ResponseInputItem> {
    let mut next_input = vec![];
    for item in items {
        if let ResponseItem::FunctionCall { .. } = item {
            next_input.push(item.into_input_item());
            let output = execute_and_wait(item);
            next_input.push(output);
        }
    }
    next_input
}
```

### 改进建议

**阶段1: 基础工具系统**
```rust
// src/tools/mod.rs (新模块)
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn spec(&self) -> ToolSpec;
    async fn execute(&self, args: Value) -> Result<ToolOutput>;
}

// 示例：Shell 工具
pub struct ShellTool;

impl Tool for ShellTool {
    fn name(&self) -> &str { "shell" }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".to_string(),
            description: "Execute shell commands".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"}
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(&self, args: Value) -> Result<ToolOutput> {
        let command = args["command"].as_str()?;
        // 执行命令（需要审批机制）
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()?;
        Ok(ToolOutput::text(String::from_utf8(output.stdout)?))
    }
}
```

**阶段2: 集成到 Responses API**
```rust
// src/services/openai.rs
fn build_responses_request(&self, tools: &[ToolSpec]) -> Value {
    json!({
        "model": self.config.model,
        "input": input,
        "instructions": instructions,
        "tools": tools.iter().map(|t| t.to_json()).collect::<Vec<_>>(),
        "tool_choice": "auto",
        "parallel_tool_calls": true,
        // ... 其他字段
    })
}
```

---

## 3️⃣ MCP (Model Context Protocol) 集成

### 当前实现

**状态**: ❌ 完全缺失

### Codex-CLI 实现

**核心组件**:

1. **McpConnectionManager** (codex-rs/core/src/mcp_connection_manager.rs)
```rust
pub struct McpConnectionManager {
    clients: HashMap<String, RmcpClient>,
    tools: Vec<McpToolInfo>,
}

impl McpConnectionManager {
    pub async fn connect_all(&mut self, configs: Vec<McpServerConfig>) {
        for config in configs {
            let client = RmcpClient::new(config.transport).await?;
            client.initialize(capabilities).await?;
            let tools = client.list_tools().await?;

            // 限定名：mcp__{server}__{tool}
            for tool in tools {
                let qualified_name = format!("mcp__{}__{}",
                    config.name,
                    tool.name.chars().take(50).collect::<String>()
                );
                self.tools.push(McpToolInfo { qualified_name, tool });
            }

            self.clients.insert(config.name, client);
        }
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> Result<CallToolResult> {
        let (server, tool) = parse_mcp_tool_name(name)?;
        let client = self.clients.get(server)?;
        client.call_tool(tool, args).await
    }
}
```

2. **工具命名规则**
```rust
// 限定名格式：mcp__{server}__{tool}
// 示例：mcp__docs__search, mcp__filesystem__read_file

fn parse_mcp_tool_name(name: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = name.strip_prefix("mcp__")?.split("__").collect();
    Ok((parts[0], parts[1]))
}
```

3. **OAuth 凭证管理**
```rust
pub enum OAuthStorage {
    Memory,
    SystemKeychain,
    File(PathBuf),
}

// 避免明文凭证暴露
```

### 改进建议

**阶段1: MCP 客户端基础**
```rust
// src/mcp/mod.rs (新模块)
pub struct McpManager {
    servers: HashMap<String, McpServer>,
}

pub struct McpServer {
    name: String,
    client: RmcpClient,
    tools: Vec<ToolSpec>,
}

impl McpManager {
    pub async fn add_server(&mut self, config: McpServerConfig) -> Result<()> {
        // 启动MCP服务器（stdio/HTTP）
        let client = connect_mcp_server(config.transport).await?;

        // 初始化并列出工具
        client.initialize().await?;
        let tools = client.list_tools().await?;

        // 转换为工具规范（添加限定名）
        let tool_specs = tools.into_iter()
            .map(|t| self.qualify_tool(&config.name, t))
            .collect();

        self.servers.insert(config.name, McpServer {
            name: config.name,
            client,
            tools: tool_specs,
        });

        Ok(())
    }

    fn qualify_tool(&self, server: &str, tool: McpTool) -> ToolSpec {
        ToolSpec {
            name: format!("mcp__{}__{}",
                server,
                tool.name.chars().take(40).collect::<String>()
            ),
            description: tool.description,
            parameters: tool.input_schema,
        }
    }
}
```

**阶段2: 工具调用路由**
```rust
// 在 ToolRouter 中添加 MCP 分支
pub async fn route_tool_call(&self, name: &str, args: Value) -> Result<ToolOutput> {
    if name.starts_with("mcp__") {
        // 路由到 MCP
        self.mcp_manager.call_tool(name, args).await
    } else {
        // 路由到本地工具
        self.local_tools.execute(name, args).await
    }
}
```

---

## 4️⃣ 请求格式完整性

### 当前实现 (src/services/openai.rs:389-398)

```rust
let request_body = json!({
    "model": &self.config.model,
    "input": input.trim(),
    "instructions": instructions,
    "reasoning": {
        "effort": self.config.reasoning_effort,
        "summary": self.config.reasoning_summary,
    },
    "stream": true,
});
```

**缺失字段**:
- ❌ `tools` - 工具定义数组
- ❌ `tool_choice` - 工具选择策略
- ❌ `parallel_tool_calls` - 并行工具调用
- ❌ `text.verbosity` / `text.format` - GPT-5 文本控制
- ❌ `include` - 请求额外字段（如 encrypted_content）
- ❌ `prompt_cache_key` - 会话/线程标识（幂等性）
- ❌ `store` - Azure 兼容性（必须为 true）

### Codex-CLI 实现 (参考文档第50-76行)

```rust
{
  "model": "gpt-5-codex",
  "instructions": "<BASE + DEV + USER 指令拼装后的大字符串>",
  "input": [
    { "type": "message", "role": "user", "content": [{"type":"input_text","text":"..."}] },
    { "type": "reasoning", "summary": [{"type":"summary_text","text":"..."}] }
  ],
  "tools": [
    { "type":"function", "name":"shell", "parameters": {...} },
    { "type":"function", "name":"mcp__docs__search", "parameters": {...} }
  ],
  "tool_choice": "auto",
  "parallel_tool_calls": true,
  "reasoning": { "effort": "medium", "summary": "detailed" },
  "text": { "verbosity": "low", "format": {"type":"json_schema","strict":true,...}},
  "store": false,
  "stream": true,
  "include": ["reasoning.encrypted_content"],
  "prompt_cache_key": "<conversation_id>"
}
```

### 改进建议

```rust
// 完整的请求构建
fn build_full_request(&self, conversation: &Conversation, tools: &[ToolSpec]) -> Value {
    let mut request = json!({
        "model": self.config.model,
        "instructions": self.build_instructions(conversation),
        "input": self.build_input(conversation),
        "stream": true,
        "prompt_cache_key": conversation.id.to_string(),
    });

    // 推理配置（仅当模型支持）
    if self.supports_reasoning() {
        request["reasoning"] = json!({
            "effort": self.config.reasoning_effort,
            "summary": self.config.reasoning_summary,
        });
        request["include"] = json!(["reasoning.encrypted_content"]);
    }

    // 工具配置
    if !tools.is_empty() {
        request["tools"] = json!(tools.iter().map(|t| t.to_json()).collect::<Vec<_>>());
        request["tool_choice"] = json!("auto");
        request["parallel_tool_calls"] = json!(true);
    }

    // Azure 兼容性
    if self.is_azure_endpoint() {
        request["store"] = json!(true);
        // 附加 item IDs
    }

    // GPT-5 文本控制
    if self.config.verbosity.is_some() {
        request["text"] = json!({
            "verbosity": self.config.verbosity,
        });
    }

    request
}
```

---

## 5️⃣ 错误处理与重试

### 当前实现

```rust
// src/services/openai.rs:422-426
if !response.status().is_success() {
    let status = response.status();
    let error_text = response.text().await?;
    return Err(anyhow::anyhow!("API 请求失败 ({}): {}", status, error_text));
}
```

**问题**:
- ❌ 无重试机制
- ❌ 无超时检测
- ❌ 无指数退避
- ❌ 不解析错误体中的重试建议
- ❌ 无幂等性保证

### Codex-CLI 实现

**重试策略** (codex-rs/core/src/client.rs:300+)
```rust
// 初次请求错误
match status {
    401 => {
        // ChatGPT 模式：尝试刷新令牌
        if self.is_chatgpt_mode() {
            self.refresh_token().await?;
            return self.retry_request(request).await;  // 重试一次
        }
    }
    429 => {
        // 解析 usage_limit_reached / usage_not_included
        let error_body = parse_error_body(response).await?;
        if let Some(retry_after) = error_body.retry_after {
            return Err(RateLimitError {
                retry_after,
                window_reset: error_body.window_reset,
            });
        }
    }
    _ => {
        // 记录 request_id (cf-ray) 用于诊断
        let request_id = response.headers().get("cf-ray");
        log_error(status, error_body, request_id);
    }
}

// 流式重试
for attempt in 0..stream_max_retries {
    match stream.next().await {
        Some(Ok(event)) => { /* 处理 */ }
        Some(Err(e)) => {
            if attempt < stream_max_retries - 1 {
                let backoff = exponential_backoff(attempt);
                sleep(backoff).await;
                continue;  // 重新建立流
            }
        }
        None => {
            // 流关闭前未见 response.completed
            if !saw_completed {
                return Err("stream closed before response.completed");
            }
        }
    }
}

// 空闲超时检测
timeout(idle_timeout, stream.next()).await?
```

### 改进建议

```rust
// src/services/retry.rs (新文件)
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub idle_timeout_ms: u64,
}

pub async fn call_with_retry<F, T>(
    config: &RetryConfig,
    mut f: F,
) -> Result<T>
where
    F: FnMut() -> BoxFuture<'static, Result<T>>,
{
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < config.max_retries => {
                // 指数退避
                let backoff = std::cmp::min(
                    config.initial_backoff_ms * 2u64.pow(attempt),
                    config.max_backoff_ms,
                );
                tokio::time::sleep(Duration::from_millis(backoff)).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}
```

---

## 6️⃣ 速率限制与遥测

### 当前实现

**状态**: ❌ 完全缺失

### Codex-CLI 实现

**RateLimitSnapshot** (codex-rs/core/src/client.rs)
```rust
pub struct RateLimitSnapshot {
    pub requests: WindowInfo,
    pub tokens: WindowInfo,
}

pub struct WindowInfo {
    pub limit: u64,
    pub remaining: u64,
    pub reset_at: SystemTime,
}

// 解析响应头
fn parse_rate_limit_snapshot(headers: &HeaderMap) -> RateLimitSnapshot {
    RateLimitSnapshot {
        requests: WindowInfo {
            limit: headers.get("x-ratelimit-limit-requests")?.parse()?,
            remaining: headers.get("x-ratelimit-remaining-requests")?.parse()?,
            reset_at: parse_reset_time(headers.get("x-ratelimit-reset-requests")?)?,
        },
        tokens: WindowInfo {
            limit: headers.get("x-ratelimit-limit-tokens")?.parse()?,
            remaining: headers.get("x-ratelimit-remaining-tokens")?.parse()?,
            reset_at: parse_reset_time(headers.get("x-ratelimit-reset-tokens")?)?,
        },
    }
}

// 立即发送事件
emit(ResponseEvent::RateLimits(snapshot));
```

**遥测** (参考文档第289行)
- 请求开始/结束时间
- SSE 事件延迟
- Token 计数（输出 token、推理 token）
- 工具调用耗时

### 改进建议

```rust
// src/telemetry/mod.rs (新模块)
pub struct TelemetryManager {
    events: Vec<TelemetryEvent>,
}

pub enum TelemetryEvent {
    RequestStarted { timestamp: SystemTime },
    RequestCompleted { duration: Duration, token_usage: TokenUsage },
    SseEventReceived { event_type: String, latency: Duration },
    ToolCallStarted { tool: String },
    ToolCallCompleted { tool: String, duration: Duration },
    RateLimitUpdate { snapshot: RateLimitSnapshot },
}

// UI 中显示速率限制
fn render_rate_limit_indicator(&self) -> impl IntoElement {
    if let Some(limits) = &self.rate_limits {
        div()
            .text_xs()
            .child(format!(
                "🚦 Requests: {}/{}",
                limits.requests.remaining,
                limits.requests.limit
            ))
    }
}
```

---

## 7️⃣ 数据模型结构化

### 当前实现 (src/models/message.rs)

```rust
pub struct Message {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
    pub reasoning_summary: Option<String>,
    pub reasoning_duration: Option<f64>,
}
```

**问题**:
- ❌ 扁平结构，无法表示复杂的响应项
- ❌ 无法区分 FunctionCall vs FunctionCallOutput
- ❌ 无法表示富媒体（图片）
- ❌ reasoning 字段耦合在 Message 中

### Codex-CLI 实现

```rust
// codex-rs/protocol/src/models.rs
pub enum ResponseItem {
    Message {
        role: Role,
        content: Vec<ContentBlock>,
    },
    Reasoning {
        summary: Vec<ReasoningText>,
        content: Option<Vec<ReasoningText>>,
        encrypted_content: Option<String>,
    },
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,  // 字符串化的 JSON
    },
    LocalShellCall {
        call_id: String,
        command: String,
    },
    FunctionCallOutput {
        call_id: String,
        output: FunctionCallOutputPayload,
    },
}

pub enum ContentBlock {
    Text(String),
    Image { image_url: String },
}

pub enum FunctionCallOutputPayload {
    Text(String),
    Structured {
        content_items: Vec<ContentBlock>,
    },
}
```

### 改进建议

```rust
// src/models/response_item.rs (新文件)
#[derive(Clone, Serialize, Deserialize)]
pub enum ResponseItem {
    Message {
        role: String,
        content: String,
    },
    Reasoning {
        summary: String,
        raw_content: Option<String>,
    },
    ToolCall {
        call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    ToolOutput {
        call_id: String,
        output: String,
        success: bool,
    },
}

// 对话历史现在存储 ResponseItem 数组
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub items: Vec<ResponseItem>,  // 替代 messages
    pub created_at: i64,
    pub updated_at: i64,
}
```

---

## 🎯 改进路线图

### Phase 1: 基础架构强化 (1-2周)

**目标**: 建立事件驱动基础，支持完整 SSE 事件

**任务**:
1. ✅ 创建 `ResponseEvent` 枚举
2. ✅ 创建 `ResponseItem` 数据模型
3. ✅ 重构 SSE 解析器（映射所有事件）
4. ✅ 处理 `response.output_item.done`
5. ✅ 支持 `reasoning_text.delta`（raw reasoning）
6. ✅ 添加 `response.completed` 一致性检查

**验收标准**:
- [ ] 能正确解析所有 Responses API SSE 事件
- [ ] 事件通过类型安全的枚举传递
- [ ] 支持推理摘要的分段显示（ReasoningSummaryPartAdded）

### Phase 2: 工具调用系统 (2-3周)

**目标**: 实现本地工具注册、路由、执行

**任务**:
1. ✅ 创建 `Tool` trait 和 `ToolRegistry`
2. ✅ 实现基础工具（Shell、Web Search）
3. ✅ 创建 `ToolRouter`（路由工具调用）
4. ✅ 创建 `ToolRuntime`（执行+审批机制）
5. ✅ 工具输出序列化（支持富媒体）
6. ✅ 工具回传与下一轮请求

**验收标准**:
- [ ] 能够定义并注册新工具
- [ ] 模型能够调用工具并获得结果
- [ ] 支持多轮对话（工具 → 模型 → 工具）
- [ ] 有审批机制（危险操作需要用户确认）

### Phase 3: MCP 集成 (2-3周)

**目标**: 连接外部 MCP 服务器，扩展工具能力

**任务**:
1. ✅ 实现 `McpConnectionManager`
2. ✅ 支持 stdio 和 HTTP 传输
3. ✅ 工具发现与限定名
4. ✅ 工具过滤（allowlist/denylist）
5. ✅ OAuth 凭证管理
6. ✅ 集成到工具路由器

**验收标准**:
- [ ] 能够连接到 MCP 服务器（如 filesystem、docs）
- [ ] 模型能够调用 MCP 工具
- [ ] 工具名无冲突（限定名机制）
- [ ] 支持配置文件管理 MCP 服务器

### Phase 4: 可靠性与可观测性 (1-2周)

**目标**: 生产级错误处理和监控

**任务**:
1. ✅ 实现重试机制（指数退避）
2. ✅ 添加空闲超时检测
3. ✅ 解析速率限制响应头
4. ✅ 实现遥测收集
5. ✅ 添加日志与诊断

**验收标准**:
- [ ] 网络抖动时自动重试
- [ ] 速率限制信息在 UI 中可见
- [ ] 有详细的诊断日志（包含 request_id）
- [ ] 长时间无响应时触发超时

### Phase 5: 高级功能 (可选)

**任务**:
1. ⭐ Responses API 代理（隔离 API Key）
2. ⭐ Azure 端点兼容
3. ⭐ GPT-5 文本控制（verbosity、JSON Schema 输出）
4. ⭐ 会话分叉与恢复（基于 prompt_cache_key）

---

## 📝 即时行动项

### 高优先级（本周）

1. **创建事件抽象层**
   - 文件: `src/services/events.rs`
   - 定义 `ResponseEvent` 和 `ResponseItem`
   - 重构 SSE 解析循环

2. **处理 output_item.done 事件**
   - 当前代码只处理 delta，忽略 item.done
   - 这是工具调用的前置条件

3. **支持 raw reasoning**
   - 添加 `response.reasoning_text.delta` 处理
   - UI 中允许切换 summary vs raw

### 中优先级（下周）

4. **设计工具调用架构**
   - 创建 `src/tools/mod.rs`
   - 定义 `Tool` trait
   - 实现 `ShellTool` 作为示例

5. **完整化请求格式**
   - 添加 `tools` 字段
   - 添加 `prompt_cache_key`
   - 添加 Azure 兼容逻辑

### 低优先级（后续迭代）

6. **实现 MCP 连接管理器**
7. **添加速率限制监控**
8. **实现遥测系统**

---

## 🔗 参考资源

- **Codex-CLI 参考文档**: `/Users/rechtan/rust-projects/gpui-test/responses-api-deep-dive.md`
- **Codex-CLI 源码**:
  - 事件处理: `codex-rs/core/src/client.rs`
  - 工具系统: `codex-rs/core/src/tools/**/*`
  - MCP 集成: `codex-rs/core/src/mcp_connection_manager.rs`
- **OpenAI Responses API 文档**: https://platform.openai.com/docs/api-reference/responses

---

## 总结

当前实现已经完成了 **Responses API 的核心功能**，包括流式输出和推理摘要显示。然而，要达到 Codex-CLI 的生产级标准，需要系统性地引入：

1. **事件驱动架构** - 完整的 SSE 事件处理
2. **工具调用系统** - 本地工具注册与执行
3. **MCP 集成** - 外部工具协议支持
4. **可靠性机制** - 重试、超时、速率限制

这些改进将使应用能够真正支持 **MCP Server 调用**，并为复杂的 AI Agent 工作流打下坚实基础。

建议按照 **Phase 1 → Phase 2 → Phase 3** 的顺序逐步实施，每个阶段都有明确的验收标准和可交付成果。
