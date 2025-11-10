# GPUI Chat 配置指南

> **版本**: 0.1.0
> **更新日期**: 2025-11-07

---

## 📋 目录

1. [概述](#概述)
2. [配置文件位置](#配置文件位置)
3. [配置文件结构](#配置文件结构)
4. [Provider 配置](#provider-配置)
5. [MCP 配置](#mcp-配置)
6. [会话配置](#会话配置)
7. [UI 配置](#ui-配置)
8. [存储配置](#存储配置)
9. [使用示例](#使用示例)
10. [最佳实践](#最佳实践)
11. [故障排除](#故障排除)

---

## 概述

GPUI Chat 使用 **TOML 格式**的配置文件替代环境变量，提供更集中、更易于管理的配置方式。

### 为什么使用配置文件？

✅ **集中管理** - 所有配置在一个文件中
✅ **类型安全** - TOML 格式提供结构化配置
✅ **多 Provider** - 轻松管理多个 AI Provider
✅ **版本控制友好** - 易于跟踪配置变更
✅ **易于分享** - 团队可以共享配置模板

### 从环境变量迁移

旧的环境变量配置方式已被弃用，请使用配置文件。

| 环境变量 | 配置文件位置 |
|---------|-------------|
| `OPENAI_API_KEY` | `providers.config.api_key` |
| `OPENAI_API_BASE` | `providers.config.api_base` |
| `OPENAI_MODEL` | `providers.config.model` |
| `OPENAI_USE_RESPONSES_API` | `providers.config.responses_api.enabled` |
| `ENABLE_MCP` | `mcp.enabled` |

---

## 配置文件位置

配置文件加载优先级（从高到低）：

### 1. 命令行指定

```bash
gpui-test --config /path/to/config.toml
```

### 2. 当前目录

```bash
./config.toml
```

### 3. 用户配置目录

**macOS**:
```
~/.config/gpui-test/config.toml
```

**Linux**:
```
~/.config/gpui-test/config.toml
```

**Windows**:
```
%APPDATA%\gpui-test\config.toml
```

### 4. 默认配置

如果未找到配置文件，将使用内置的默认配置。

---

## 配置文件结构

### 基本结构

```toml
[app]
name = "GPUI Chat"
version = "0.1.0"

[[providers]]
name = "openai-default"
type = "openai"
enabled = true

[providers.config]
api_key = "sk-..."
model = "gpt-4"

[mcp]
enabled = false

[conversation]
max_history = 100

[ui]
theme = "light"

[storage]
encrypt_api_keys = false
```

### 配置节说明

| 节 | 必需 | 说明 |
|----|------|------|
| `[app]` | ❌ | 应用信息 |
| `[[providers]]` | ✅ | Provider 配置（至少一个） |
| `[mcp]` | ❌ | MCP 集成配置 |
| `[conversation]` | ❌ | 会话管理配置 |
| `[ui]` | ❌ | UI 配置 |
| `[storage]` | ❌ | 存储配置 |

---

## Provider 配置

### 支持的 Provider 类型

- **OpenAI** - OpenAI API
- **AzureOpenAI** - Azure OpenAI Service
- **Anthropic** - Anthropic Claude
- **Custom** - 自定义 API 端点

### OpenAI Provider

```toml
[[providers]]
name = "openai-default"
type = "openai"
enabled = true

[providers.config]
# API Key（必填）
api_key = "sk-your-api-key-here"

# API Base URL（可选）
# api_base = "https://api.openai.com/v1"

# 模型名称
model = "gpt-4"

# Responses API 配置
[providers.config.responses_api]
enabled = true
reasoning_effort = "medium"  # low, medium, high
reasoning_summary = "auto"   # auto, always, never

# 工具配置
[providers.config.tools]
enabled = true
approval_policy = "safe"  # auto, safe, require
max_tool_rounds = 5

# 重试配置
[providers.config.retry]
max_attempts = 3
base_delay_ms = 1000
max_delay_ms = 60000
jitter_ms = 500

# 超时配置
[providers.config.timeout]
request_timeout_secs = 120
stream_idle_timeout_secs = 30

# 遥测配置
[providers.config.telemetry]
enabled = false

# 生成配置
[providers.config.generation]
preset = "balanced"  # precise, balanced, creative, concise, detailed
```

### Azure OpenAI Provider

```toml
[[providers]]
name = "azure-openai"
type = "azureopenai"
enabled = true

[providers.config]
api_key = "your-azure-api-key"
api_base = "https://your-resource.openai.azure.com"
model = "gpt-4"

# Azure 特定配置
[providers.config.extra]
api_version = "2024-02-15-preview"
deployment_name = "gpt-4-deployment"
```

### Anthropic Claude Provider

```toml
[[providers]]
name = "anthropic"
type = "anthropic"
enabled = false

[providers.config]
api_key = "your-anthropic-api-key"
model = "claude-3-opus-20240229"
```

### 多 Provider 配置

可以配置多个 Provider 并根据需要切换：

```toml
# 默认 Provider（快速响应）
[[providers]]
name = "gpt-3.5"
type = "openai"
enabled = true

[providers.config]
api_key = "sk-..."
model = "gpt-3.5-turbo"

# 高级 Provider（高质量）
[[providers]]
name = "gpt-4"
type = "openai"
enabled = false  # 默认不使用

[providers.config]
api_key = "sk-..."
model = "gpt-4"

# 创意 Provider（创作任务）
[[providers]]
name = "creative"
type = "openai"
enabled = false

[providers.config]
api_key = "sk-..."
model = "gpt-4"

[providers.config.generation]
preset = "creative"
temperature = 0.9
```

---

## MCP 配置

```toml
[mcp]
# 是否启用 MCP
enabled = true

# MCP 服务器配置文件路径
servers_config = "./mcp_servers.toml"
```

如果不指定 `servers_config`，将从以下位置加载：

1. `./mcp_servers.toml`
2. `~/.config/gpui-test/mcp_servers.toml`

参见 [MCP_GUIDE.md](MCP_GUIDE.md) 了解详细的 MCP 配置。

---

## 会话配置

```toml
[conversation]
# 最大会话历史数量
max_history = 100

# 保留的快照数量
keep_snapshots = 20

# 自动快照配置
[conversation.auto_snapshot]
enabled = true
interval = 10  # 每 10 条消息自动创建快照
```

### 自动快照

当启用自动快照时，系统会在每 N 条消息后自动创建会话快照，方便恢复到之前的状态。

### 快照清理

当快照数量超过 `keep_snapshots` 时，旧快照将被自动删除，只保留最近的快照。

---

## UI 配置

```toml
[ui]
# 主题
theme = "light"  # light, dark

# 字体大小
font_size = 14.0

# 窗口大小
window_width = 1200.0
window_height = 800.0
```

---

## 存储配置

```toml
[storage]
# 数据目录（可选）
# data_dir = "~/.local/share/gpui-test"

# 是否加密 API Key（后续功能）
encrypt_api_keys = false
```

### API Key 加密

**当前**: API Key 以明文存储在配置文件中
**计划**: 后续版本将支持本地加密存储

**安全建议**:
- ✅ 将 `config.toml` 添加到 `.gitignore`
- ✅ 设置文件权限为 `600`（仅所有者可读写）
- ✅ 不要分享配置文件
- ✅ 定期轮换 API Key

---

## 使用示例

### 示例 1: 基本配置

```toml
[app]
name = "My Chat App"

[[providers]]
name = "openai"
type = "openai"
enabled = true

[providers.config]
api_key = "sk-..."
model = "gpt-4"
```

### 示例 2: 多 Provider + MCP

```toml
[[providers]]
name = "openai"
type = "openai"
enabled = true

[providers.config]
api_key = "sk-..."
model = "gpt-4"

[[providers]]
name = "claude"
type = "anthropic"
enabled = false

[providers.config]
api_key = "sk-ant-..."
model = "claude-3-opus-20240229"

[mcp]
enabled = true
servers_config = "./mcp_servers.toml"
```

### 示例 3: 精细控制

```toml
[[providers]]
name = "precise"
type = "openai"
enabled = true

[providers.config]
api_key = "sk-..."
model = "gpt-4"

# 精确模式配置
[providers.config.generation]
preset = "precise"
temperature = 0.3
top_p = 0.9
max_tokens = 2048

# 严格的工具审批
[providers.config.tools]
enabled = true
approval_policy = "require"

# 更激进的重试
[providers.config.retry]
max_attempts = 5
base_delay_ms = 2000
```

---

## 最佳实践

### 1. 配置管理

**推荐做法**:
```bash
# 使用版本控制跟踪配置模板
git add config.toml.example

# 不要提交实际配置
echo "config.toml" >> .gitignore

# 为不同环境创建配置
config.dev.toml
config.prod.toml
config.local.toml
```

### 2. Provider 选择

根据任务选择合适的 Provider：

| 任务类型 | 推荐 Provider | Preset |
|---------|--------------|--------|
| 事实查询 | GPT-4 | precise |
| 代码生成 | GPT-4 | precise |
| 创意写作 | GPT-4 | creative |
| 快速对话 | GPT-3.5 | balanced |
| 深度分析 | Claude Opus | detailed |

### 3. 安全配置

```toml
# 设置工具审批策略
[providers.config.tools]
approval_policy = "safe"  # 或 "require" 以获得更高安全性

# 限制工具轮数
max_tool_rounds = 3

# 启用遥测监控
[providers.config.telemetry]
enabled = true
```

### 4. 性能优化

```toml
# 调整重试策略
[providers.config.retry]
max_attempts = 3
base_delay_ms = 1000

# 设置合适的超时
[providers.config.timeout]
request_timeout_secs = 60  # 非关键任务可以更短
stream_idle_timeout_secs = 15
```

---

## 故障排除

### 配置文件未加载

**问题**: 应用使用默认配置而不是自定义配置

**解决方案**:
1. 检查配置文件位置
   ```bash
   ls -la ~/.config/gpui-test/config.toml
   ```

2. 验证文件权限
   ```bash
   chmod 600 ~/.config/gpui-test/config.toml
   ```

3. 检查文件格式
   ```bash
   # 使用 TOML 验证器
   cat config.toml | toml-lint
   ```

### API Key 错误

**问题**: "Invalid API key" 或 "Unauthorized"

**解决方案**:
1. 验证 API Key 格式
   - OpenAI: `sk-...` (以 sk- 开头)
   - Anthropic: `sk-ant-...` (以 sk-ant- 开头)

2. 检查 API Key 是否有效
   ```bash
   curl https://api.openai.com/v1/models \
     -H "Authorization: Bearer YOUR_API_KEY"
   ```

3. 确保没有多余的空格或换行符

### Provider 未启用

**问题**: Provider 配置了但未生效

**解决方案**:
1. 检查 `enabled = true`
2. 确保至少有一个 Provider 启用
3. 验证配置文件语法

### TOML 语法错误

常见错误：

```toml
# ❌ 错误：缺少引号
api_key = sk-123

# ✅ 正确
api_key = "sk-123"

# ❌ 错误：数组语法
providers = [{name = "test"}]

# ✅ 正确
[[providers]]
name = "test"
```

---

## 配置验证

### 验证配置文件

```bash
# 使用 Python 验证
python3 -c "import toml; toml.load('config.toml')"

# 使用 Rust 验证
cargo run -- --validate-config
```

### 配置模式

参考 `config.toml.example` 获取完整的配置模板。

---

## 更新日志

### 2025-11-07
- ✅ 初始版本
- ✅ 替换环境变量为 TOML 配置
- ✅ 支持多 Provider
- ✅ 完整的配置文档

---

## 相关文档

- [MCP_GUIDE.md](MCP_GUIDE.md) - MCP 集成指南
- [config.toml.example](config.toml.example) - 配置文件示例
- [PROJECT_STATUS.md](PROJECT_STATUS.md) - 项目状态
- [PHASE5_COMPLETION_REPORT.md](PHASE5_COMPLETION_REPORT.md) - Phase 5 报告

---

**配置系统版本**: 1.0
**最后更新**: 2025-11-07
