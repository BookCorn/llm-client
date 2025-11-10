# Phase 2 进度报告：工具调用系统

> **开始时间**: 2025-11-07
> **当前状态**: 🟡 进行中（80% 完成）

---

## ✅ 已完成任务

### 1. 工具系统核心架构 ✅

**创建文件** (500+ 行代码):
- `src/tools/mod.rs` - 模块入口
- `src/tools/spec.rs` - Tool trait 和工具规范
- `src/tools/registry.rs` - 工具注册表
- `src/tools/router.rs` - 工具路由器
- `src/tools/runtime.rs` - 工具执行运行时

**关键类型**:
```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn spec(&self) -> ToolSpec;
    async fn execute(&self, args: Value) -> Result<ToolOutput>;
    fn requires_approval(&self) -> bool { false }
    fn approval_prompt(&self, args: &Value) -> String;
}

pub struct ToolRegistry; // 工具注册表
pub struct ToolRouter;   // 工具路由器
pub struct ToolRuntime;  // 工具执行引擎
```

### 2. 内置工具实现 ✅

**ShellTool** (`src/tools/builtin/shell.rs`):
- 执行 shell 命令
- 支持审批机制
- 跨平台（Unix/Windows）
- 完整的错误处理

**测试覆盖**:
- 5 个单元测试
- 所有测试通过 ✅

### 3. 工具系统集成到 OpenAI 服务 ✅

**OpenAIServiceConfig 扩展**:
```rust
pub struct OpenAIServiceConfig {
    // ... 现有字段
    pub enable_tools: bool,
    pub tool_approval_policy: ApprovalPolicy,
}
```

**环境变量配置**:
- `OPENAI_ENABLE_TOOLS=true` - 启用工具
- `OPENAI_TOOL_APPROVAL=auto|safe|require` - 审批策略

**OpenAIService 扩展**:
```rust
pub struct OpenAIService {
    client: Client<OpenAIConfig>,
    config: OpenAIServiceConfig,
    tool_registry: Arc<ToolRegistry>,   // ✨ 新增
    tool_runtime: Arc<ToolRuntime>,     // ✨ 新增
}
```

### 4. Responses API 请求增强 ✅

**工具定义注入**:
```rust
// 在请求中添加 tools 字段
if self.config.enable_tools {
    let tool_specs = self.tool_registry.specs();
    request_body["tools"] = json!(tools_json);
    request_body["tool_choice"] = json!("auto");
    request_body["parallel_tool_calls"] = json!(true);
}
```

**输出示例**:
```
🔧 启用工具调用: 1 个工具
   - shell: Execute a shell command and return the output...
```

### 5. 测试验证 ✅

**单元测试**:
- 26 个测试全部通过
- 工具系统测试：12 个
- 事件系统测试：4 个
- 其他：10 个

**编译状态**:
- ✅ 编译成功
- 32 个警告（未使用代码，预期中）

---

## 🚧 待完成任务

### 1. 处理 OutputItemDone 事件中的工具调用 🟡

**当前状态**: 事件解析已就绪，需要添加处理逻辑

**需要做的**:
```rust
ResponseEvent::OutputItemDone(item) => {
    // TODO: 检测工具调用
    if let Some(invocation) = ToolRouter::build_tool_invocation(item)? {
        // TODO: 执行工具
        let result = self.tool_runtime.execute(invocation).await;

        // TODO: 处理结果（成功/等待审批/失败）
        match result {
            ExecutionResult::Success(tool_result) => {
                // TODO: 回传给模型
            }
            ExecutionResult::AwaitingApproval { .. } => {
                // TODO: 请求用户审批
            }
            ExecutionResult::Error { .. } => {
                // TODO: 返回错误
            }
        }
    }
}
```

**挑战**:
- 需要暂停当前响应流
- 执行工具后需要发起新的请求
- 涉及复杂的异步控制流

### 2. 实现工具执行与回传 🔴

**设计方案**:

**方案 A: 同步阻塞**
- 遇到工具调用时停止流
- 执行工具（可能需要审批）
- 构建新请求（包含工具输出）
- 继续流式响应

**方案 B: 异步队列**
- 将工具调用放入队列
- 流结束后统一处理
- 适用于并行工具调用

**推荐**: 方案 A（更符合 Codex-CLI 模式）

### 3. 实现多轮对话（工具 -> 模型 -> 工具）🔴

**需要实现**:
1. 工具调用历史记录
2. 下一轮请求构建（包含工具输出）
3. 递归调用直到无工具调用

**伪代码**:
```rust
async fn execute_with_tools(&self, messages: &[Message]) -> Result<String> {
    loop {
        let (content, tool_calls) = self.call_responses_api(messages).await?;

        if tool_calls.is_empty() {
            return Ok(content);  // 没有工具调用，完成
        }

        // 执行所有工具调用
        let tool_results = self.execute_tools(tool_calls).await?;

        // 将工具输出添加到消息历史
        messages.extend(tool_calls);
        messages.extend(tool_results);

        // 继续下一轮（递归）
    }
}
```

---

## 📊 完成度统计

| 任务 | 状态 | 完成度 |
|------|------|--------|
| Tool trait 和 ToolSpec | ✅ | 100% |
| ToolRegistry | ✅ | 100% |
| ToolRouter | ✅ | 100% |
| ToolRuntime | ✅ | 100% |
| ShellTool | ✅ | 100% |
| 集成到 OpenAI 服务 | ✅ | 100% |
| 请求中添加 tools | ✅ | 100% |
| **处理工具调用** | 🟡 | **30%** |
| **工具执行与回传** | 🔴 | **0%** |
| **多轮对话** | 🔴 | **0%** |
| **总体进度** | 🟡 | **70%** |

---

## 🎯 下一步行动

### 立即任务（今天完成）

1. **实现工具调用处理器** (2-3小时)
   - 在 SSE 解析器中检测工具调用
   - 调用 ToolRouter 解析
   - 调用 ToolRuntime 执行

2. **实现简单的工具回传** (1-2小时)
   - 执行完工具后立即返回结果
   - 暂不支持多轮（先验证单次工具调用）

### 短期任务（本周）

3. **实现多轮对话** (3-4小时)
   - 递归调用模式
   - 工具输出序列化
   - 历史消息管理

4. **添加审批 UI** (2-3小时)
   - 在 main.rs 中添加审批对话框
   - 用户批准/拒绝工具调用

### 中期任务（下周）

5. **并行工具调用** (2-3小时)
   - 同时执行多个工具
   - 结果聚合

6. **工具调用可视化** (3-4小时)
   - 显示工具调用进度
   - 显示工具输出
   - 美化 UI

---

## 💡 技术难点

### 难点 1: 异步控制流

**问题**: SSE 流是连续的，如何在中间暂停执行工具？

**方案**:
```rust
// 使用 channel 通信
let (tool_tx, tool_rx) = tokio::sync::mpsc::channel(10);

// SSE 解析器发送工具调用
tool_tx.send(invocation).await?;

// 主线程接收并执行
let result = tool_runtime.execute(tool_rx.recv().await?).await;
```

### 难点 2: 流式与工具调用的协调

**问题**: 用户看到流式输出，但工具调用是阻塞的

**方案**:
- 显示"正在调用工具..."提示
- 工具执行期间暂停文本流
- 工具完成后继续流

### 难点 3: 多轮对话的消息历史

**问题**: 如何构建包含工具调用和输出的消息历史？

**参考 Codex-CLI**:
```rust
// input 数组包含：
[
    { "type": "message", "role": "user", "content": "..." },
    { "type": "function_call", "call_id": "...", "name": "...", "arguments": "..." },
    { "type": "function_call_output", "call_id": "...", "output": "..." },
]
```

---

## 🔗 参考资源

- **Codex-CLI 工具系统**: `codex-rs/core/src/tools/`
- **Responses API 文档**: 第194-217行（工具调用）
- **工具回传格式**: 第256-262行
- **多轮对话**: 第309-317行

---

## 📝 代码统计（截至目前）

| 类别 | 行数 | 文件 |
|------|------|------|
| 工具系统核心 | ~600 | 5 个 |
| 内置工具 | ~150 | 2 个 |
| 服务集成 | ~50 | openai.rs |
| 单元测试 | ~200 | 各模块 |
| **总计** | **~1000** | **9 个新文件** |

---

## 🎉 里程碑

- ✅ **Milestone 1**: 工具系统基础架构（完成）
- 🟡 **Milestone 2**: 单次工具调用（进行中）
- ⏳ **Milestone 3**: 多轮对话（待开始）
- ⏳ **Milestone 4**: 审批 UI（待开始）
- ⏳ **Milestone 5**: MCP 集成准备（Phase 3）

---

**当前瓶颈**: 工具调用处理的异步控制流设计

**预计完成时间**: Phase 2 基础功能 - 2-3 天
