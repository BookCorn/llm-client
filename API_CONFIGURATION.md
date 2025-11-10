# API 配置说明

## 问题说明

你遇到的错误是因为使用的第三方 API 服务（`https://api.ncpsnetworks.com`）不支持 OpenAI 的新 Responses API。

```
{"error":{"message":"No input provided","type":"upstream_error","param":"","code":"invalid_prompt"}}
```

## 解决方案

现在应用支持**自动检测**和**手动配置**两种方式来选择使用哪个 API：

### 方法 1: 自动检测（推荐）

默认不做任何自动检测或回退。是否使用 Responses API 完全由你控制：
- 通过环境变量 `OPENAI_USE_RESPONSES_API` 或应用右上角的开关进行切换
- 未设置时默认使用 Chat Completions API（更通用）

### 方法 2: 手动配置

使用环境变量：

```bash
# 使用 Responses API（支持 reasoning summary）
export OPENAI_USE_RESPONSES_API=true

# 使用 Chat Completions API（传统模式，更通用）
export OPENAI_USE_RESPONSES_API=false

# Responses API 推理选项（可选）
# 努力度：minimal|low|medium|high（默认 medium）
export OPENAI_REASONING_EFFORT=medium
# 摘要模式：auto|on|off（默认 auto）
export OPENAI_REASONING_SUMMARY=auto

# Phase 4 可靠性 & 可观测性
export OPENAI_RETRY_MAX_ATTEMPTS=3
export OPENAI_RETRY_BASE_DELAY_MS=500
export OPENAI_RETRY_MAX_DELAY_MS=8000
export OPENAI_RETRY_JITTER_MS=250
export OPENAI_REQUEST_TIMEOUT_MS=15000
export OPENAI_STREAM_IDLE_TIMEOUT_MS=20000
export OPENAI_TELEMETRY_ENABLED=true

# 运行应用
cargo run
```

## 两种 API 的区别（由你选择）

| 特性 | Chat Completions API | Responses API |
|------|---------------------|---------------|
| 端点 | `/v1/chat/completions` | `/v1/responses` |
| 兼容性 | ✅ 广泛兼容 | ⚠️ 视服务实现而定 |
| 推理摘要 | ❌ 不支持 | ✅ 支持 |
| 输入格式 | `messages` 数组（完整对话历史） | `input` 字符串（仅最后一条消息） |
| 推荐场景 | 第三方 API 服务 | OpenAI 官方 + 推理模型 |

## 环境变量完整列表

```bash
# 必需：API 密钥
export OPENAI_API_KEY="your-api-key"

# 可选：自定义 API 端点（默认：https://api.openai.com/v1）
export OPENAI_API_BASE="https://api.ncpsnetworks.com/v1"

# 可选：模型（默认：gpt-4）
export OPENAI_MODEL="gpt-5-mini"

# 可选：API 类型选择（默认：自动检测）
export OPENAI_USE_RESPONSES_API=false  # 强制使用 Chat Completions API
```

## 选择策略建议

- 知道你的端点支持 Responses 时，设置 `OPENAI_USE_RESPONSES_API=true` 或在 UI 开关打开。
- 不确定或只需要传统功能时，设置为 `false`（默认）。

## 日志信息说明

### 使用 Chat Completions API 时：

```
ℹ️ 使用 Chat Completions API（传统模式，不支持 reasoning summary）
🔗 调用 Chat Completions API: https://api.ncpsnetworks.com/v1/chat/completions
📦 输出delta: 12 字符
✅ Chat Completions API 完成 - 输出: 156 字符
```

### 使用 Responses API 时：

```
ℹ️ 使用 Responses API（支持 reasoning summary）
🔗 使用 Responses API 调用: https://api.openai.com/v1/responses
📝 模型: gpt-5-mini
🧠 推理模式: summary=auto
🧠 推理摘要delta: 25 字符
📦 输出delta: 12 字符
✅ Responses API 完成 - 输出: 156 字符, 推理摘要: Some("87 字符")
```

## 测试

### 1. 测试第三方 API（Chat Completions）

```bash
export OPENAI_API_KEY="your-key"
export OPENAI_API_BASE="https://api.ncpsnetworks.com/v1"
export OPENAI_MODEL="gpt-5-mini"
export OPENAI_USE_RESPONSES_API=false

cargo run
```

### 2. 测试支持 Responses 的端点

```bash
export OPENAI_API_KEY="your-openai-key"
export OPENAI_MODEL="gpt-5-mini"
export OPENAI_USE_RESPONSES_API=true

cargo run
```

## 故障排除

### 问题 1：仍然报错 "No input provided"

请确认你的端点对 `/v1/responses` 的协议兼容性；如需切换回传统模式：
`export OPENAI_USE_RESPONSES_API=false`

### 问题 2：想要看到推理过程

**解决**：使用 OpenAI 官方 API + 推理模型（gpt-5-mini, o1, o3 等）

```bash
export OPENAI_API_KEY="your-official-openai-key"
export OPENAI_MODEL="gpt-5-mini"
unset OPENAI_API_BASE  # 使用官方端点
cargo run
```

### 问题 3：不确定使用了哪个 API

**查看日志**：运行应用时会显示：
- `ℹ️ 使用 Chat Completions API...` 或
- `ℹ️ 使用 Responses API...`

## 总结

应用不再进行“是否支持 Responses”的自动判断或回退。请用开关自行选择最合适的 API。

---

**更新日期**: 2025-01-05
**状态**: ✅ 已实现自动 API 选择
