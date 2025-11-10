# 推理模型支持说明

## 当前状态 ✅

本应用**已完成** Responses API 集成，可以完整提取和显示推理摘要（Reasoning Summary）！

## 实现方案

### 模式选择：Responses API 或 Chat Completions

本应用同时支持 **OpenAI Responses API** 与 **Chat Completions API**。是否启用 Responses 由你显式控制：

- 运行时右上角“Responses API: ON/OFF” 开关
- 或通过环境变量 `OPENAI_USE_RESPONSES_API`（true/false）

### Responses API vs Chat Completions API

| 特性 | Chat Completions API | Responses API |
|------|---------------------|---------------|
| 端点 | `/v1/chat/completions` | `/v1/responses` |
| 推理摘要 | ❌ 不支持 | ✅ 支持 |
| 输入格式 | `messages` 数组 | `input` 字符串 |
| SSE 格式 | 仅 `data:` 行 | `event:` + `data:` 行 |
| 推理字段 | `reasoning_content` (不可靠) | `reasoning_summary_text` ✅ |

### Responses API SSE 事件类型

Responses API 使用标准 SSE 格式，包含多种事件类型：

```
event: response.reasoning_summary_text.delta
data: {"delta": "推理摘要的增量文本..."}

event: response.output_text.delta
data: {"delta": "输出内容的增量文本..."}

event: response.completed
data: {...}
```

## 当前行为（由你的选择决定）

- ✅ **普通模型**（如 gpt-4, gpt-3.5-turbo）：正常工作，不显示推理过程
- ✅ **推理模型**（如 gpt-5-mini, o1, o3 系列）：
  - ✅ 正常显示 AI 回复
  - ✅ 完整提取和显示推理摘要
  - ✅ 实时流式显示推理过程
  - ✅ 数据驱动：只在 API 返回推理数据时显示

## 技术实现细节

### 1. 请求格式

```rust
let request_body = json!({
    "model": "gpt-5-mini",
    "input": "用户消息",
    "reasoning": {
        "summary": "auto"
    },
    "stream": true
});
```

### 2. SSE 解析

```rust
// 跟踪当前事件类型
let mut current_event = String::new();

// 先解析 event: 行
if let Some(event_type) = line_str.strip_prefix("event: ") {
    current_event = event_type.to_string();
}

// 再解析 data: 行
if let Some(data) = line_str.strip_prefix("data: ") {
    match current_event.as_str() {
        "response.reasoning_summary_text.delta" => {
            // 处理推理摘要
            if let Some(delta) = json["delta"].as_str() {
                reasoning_summary.push_str(delta);
                on_reasoning(delta.to_string());
            }
        }
        "response.output_text.delta" => {
            // 处理输出文本
            if let Some(delta) = json["delta"].as_str() {
                full_response.push_str(delta);
                on_chunk(delta.to_string());
            }
        }
        _ => {}
    }
}
```

### 3. 双回调系统

```rust
openai.get_streaming_completion_native(
    &messages,
    // 回调1: 处理输出文本
    |chunk| { /* 更新 UI */ },
    // 回调2: 处理推理摘要
    |reasoning| { /* 更新推理框 */ }
).await
```

## 架构优势

✅ **完整功能**：使用 Responses API 实现了完整的推理摘要提取
✅ **数据驱动**：只显示实际接收到的数据，不预设或伪造
✅ **条件渲染**：`self.is_reasoning && self.reasoning_summary.is_some()`
✅ **实时流式**：推理摘要和输出文本同时流式显示
✅ **为 MCP 做准备**：数据驱动架构易于集成其他数据源

## 测试推理模型

使用以下模型测试推理功能：

```bash
# 选择 API 类型
export OPENAI_USE_RESPONSES_API=true   # Responses；false 使用 Chat

# 选择模型
export OPENAI_MODEL="gpt-5-mini"      # 支持 reasoning summary 的模型
# export OPENAI_MODEL="o1-preview"     # 或 o1/o3 系列
# export OPENAI_MODEL="gpt-4"          # 普通模型（无 reasoning summary）

cargo run
```

## 控制台输出示例

### 使用推理模型：
```
🔗 使用 Responses API 调用: https://api.openai.com/v1/responses
📝 模型: gpt-5-mini
🧠 推理模式: summary=auto
🧠 检测到推理摘要！开始流式显示推理过程
🧠 推理摘要delta: 25 字符
📦 输出delta: 12 字符
✅ Responses API 完成 - 输出: 156 字符, 推理摘要: Some("87 字符")
```

### 使用普通模型：
```
🔗 使用 Responses API 调用: https://api.openai.com/v1/responses
📝 模型: gpt-4
🧠 推理模式: summary=auto
📦 输出delta: 18 字符
✅ Responses API 完成 - 输出: 203 字符, 推理摘要: None
```

## 参考资料

- [OpenAI Responses API 文档](https://platform.openai.com/docs/guides/reasoning?api-mode=responses)
- [Responses API Cookbook](https://cookbook.openai.com/examples/responses_api/reasoning_items)
- [Zed Editor 实现参考](https://github.com/zed-industries/zed/pull/36199)
- [Why We Built the Responses API](https://developers.openai.com/blog/responses-api/)

## 未来改进

- [ ] 支持对话历史（Responses API 支持 `previous_response_id`）
- [ ] 支持工具调用（MCP Server 集成）
- [ ] 支持背景处理模式（长时运行任务）
- [ ] 添加 reasoning effort 配置（Minimal/Low/Medium/High）

---

**更新日期**: 2025-01-05
**状态**: ✅ 已完成 Responses API 集成
