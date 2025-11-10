# MCP 集成指南

> **Model Context Protocol (MCP)** 集成指南

本文档介绍如何在 GPUI Chat Application 中使用 MCP（Model Context Protocol）集成。

---

## 📋 目录

- [什么是 MCP](#什么是-mcp)
- [快速开始](#快速开始)
- [配置文件](#配置文件)
- [环境变量](#环境变量)
- [支持的 MCP 服务器](#支持的-mcp-服务器)
- [工具过滤](#工具过滤)
- [故障排除](#故障排除)

---

## 什么是 MCP

**Model Context Protocol (MCP)** 是一个开放协议，允许 AI 应用与外部工具和数据源安全交互。

### 主要特性

- 🔌 **标准化工具接口**: 统一的工具调用协议
- 🔒 **安全沙箱**: 工具执行审批机制
- 🌐 **广泛生态**: 支持文件系统、Git、数据库、API 等
- 🎯 **限定命名**: 避免工具名称冲突（`mcp__{server}__{tool}`）

---

## 快速开始

### 1. 创建配置文件

复制示例配置：

```bash
cp mcp_servers.toml.example mcp_servers.toml
```

### 2. 编辑配置

编辑 `mcp_servers.toml` 并启用所需的服务器：

```toml
[[servers]]
name = "filesystem"
enabled = true  # 改为 true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/Users/yourname/Documents"]
```

### 3. 运行应用

```bash
# 启用工具调用和 MCP
export OPENAI_ENABLE_TOOLS=true
export ENABLE_MCP=true

cargo run
```

### 4. 在应用中初始化 MCP（代码示例）

```rust
// 在 main.rs 中
let mut openai_service = OpenAIService::new();

// 异步初始化 MCP
openai_service.initialize_mcp().await?;
```

---

## 配置文件

### 配置文件位置（按优先级）

1. **环境变量指定**: `MCP_SERVERS_CONFIG=/path/to/config.toml`
2. **当前目录**: `./mcp_servers.toml`
3. **用户配置目录**: `~/.config/gpui-test/mcp_servers.toml`

### 配置文件结构

```toml
[[servers]]
name = "server-name"        # 服务器名称（用于工具限定名）
enabled = true              # 是否启用

[servers.connection_type]
type = "stdio"              # 连接类型: stdio 或 http

[servers.connection_params]
command = "command"         # 命令
args = ["arg1", "arg2"]     # 参数
env = { KEY = "value" }     # 环境变量（可选）

[servers.tool_filter]
allow = []                  # 允许列表（空=允许全部）
deny = []                   # 拒绝列表
```

---

## 环境变量

### MCP 相关

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `ENABLE_MCP` | 启用/禁用 MCP 集成 | `true` |
| `MCP_SERVERS_CONFIG` | 配置文件路径 | 自动查找 |

### 工具系统相关

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `OPENAI_ENABLE_TOOLS` | 启用工具调用 | `false` |
| `OPENAI_TOOL_APPROVAL` | 审批策略：`auto`/`safe`/`require` | `safe` |

### 示例

```bash
# 完整启用工具和 MCP
export OPENAI_ENABLE_TOOLS=true
export ENABLE_MCP=true
export OPENAI_TOOL_APPROVAL=safe

# 使用自定义配置文件
export MCP_SERVERS_CONFIG=/path/to/my-mcp-config.toml

cargo run
```

---

## 支持的 MCP 服务器

### 官方服务器

#### 1. Filesystem

访问本地文件系统（只读或读写）。

```toml
[[servers]]
name = "filesystem"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/directory"]
```

**可用工具**:
- `read_file` - 读取文件内容
- `write_file` - 写入文件
- `list_directory` - 列出目录
- `create_directory` - 创建目录

#### 2. Git

与 Git 仓库交互。

```toml
[[servers]]
name = "git"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-git"]
```

**可用工具**:
- `git_status` - 查看状态
- `git_diff` - 查看差异
- `git_log` - 查看日志
- `git_commit` - 提交更改

#### 3. GitHub

访问 GitHub API。

```toml
[[servers]]
name = "github"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]

[servers.connection_params.env]
GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_your_token_here"
```

**可用工具**:
- `search_repositories` - 搜索仓库
- `get_file_contents` - 获取文件内容
- `create_issue` - 创建 Issue
- `create_pull_request` - 创建 PR

#### 4. PostgreSQL

查询 PostgreSQL 数据库。

```toml
[[servers]]
name = "postgres"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres", "postgresql://user:pass@localhost/db"]
```

**可用工具**:
- `query` - 执行 SQL 查询
- `list_tables` - 列出表
- `describe_table` - 描述表结构

---

## 工具过滤

### 允许列表（Allowlist）

仅允许特定工具：

```toml
[servers.tool_filter]
allow = ["read_file", "list_directory"]
deny = []
```

### 拒绝列表（Denylist）

拒绝特定工具：

```toml
[servers.tool_filter]
allow = []  # 空=允许全部
deny = ["delete_file", "execute_query"]
```

### 通配符支持

```toml
[servers.tool_filter]
allow = ["read_*", "list_*"]          # 允许所有读取和列表操作
deny = ["*_delete", "*_dangerous"]     # 拒绝所有删除和危险操作
```

### 优先级

1. **拒绝列表优先**: 在拒绝列表中的工具总是被拒绝
2. **允许列表检查**: 如果允许列表为空，允许所有（除了被拒绝的）
3. **默认拒绝**: 如果允许列表不为空，只允许列表中的工具

---

## 故障排除

### 问题：找不到 MCP 配置文件

**症状**:
```
ℹ️  未找到 MCP 配置文件，跳过 MCP 集成
```

**解决方案**:
1. 确保配置文件存在于正确位置
2. 检查文件名是否正确（`mcp_servers.toml`）
3. 或设置环境变量：`export MCP_SERVERS_CONFIG=/path/to/config.toml`

### 问题：MCP 服务器连接失败

**症状**:
```
❌ 连接到 filesystem 失败: Failed to spawn MCP server
```

**解决方案**:
1. 检查命令是否存在：`which npx`
2. 测试命令手动运行：`npx -y @modelcontextprotocol/server-filesystem /tmp`
3. 检查参数是否正确（特别是路径）

### 问题：工具未注册

**症状**:
```
⏭️  跳过被过滤的工具: dangerous_tool
```

**解决方案**:
1. 检查 `tool_filter` 配置
2. 确保工具不在 `deny` 列表中
3. 如果使用 `allow` 列表，确保工具在列表中

### 问题：MCP 集成被禁用

**症状**:
```
ℹ️  MCP 集成已禁用（ENABLE_MCP=false）
```

**解决方案**:
```bash
export ENABLE_MCP=true
```

### 调试技巧

#### 1. 启用详细日志

在代码中，所有 MCP 操作都会打印日志：

```
🔌 初始化 MCP 集成...
✅ 加载到 2 个 MCP 服务器配置
🔌 开始连接到 MCP 服务器...
✅ 连接到 MCP 服务器: filesystem
🔍 开始发现 MCP 工具...
✅ 从 filesystem 发现 4 个工具
🎉 MCP 集成完成！总共 4 个工具可用
```

#### 2. 检查工具注册

在代码中调用：

```rust
let specs = openai_service.tool_registry.specs();
for spec in specs {
    println!("已注册工具: {}", spec.name);
}
```

#### 3. 测试单个服务器

创建最小配置文件：

```toml
[[servers]]
name = "test"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "echo"
args = []
```

---

## 高级用法

### 自定义 MCP 服务器

如果你有自己的 MCP 服务器实现：

```toml
[[servers]]
name = "my-server"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "/path/to/my-mcp-server"
args = ["--config", "/path/to/config.json"]

[servers.connection_params.env]
MY_API_KEY = "secret"
DEBUG = "true"
```

### 工具限定名

所有 MCP 工具使用限定名格式：`mcp__{server}__{tool}`

例如：
- `mcp__filesystem__read_file`
- `mcp__git__git_status`
- `mcp__github__search_repositories`

这避免了不同服务器之间的工具名称冲突。

---

## 资源

- [MCP 官方文档](https://modelcontextprotocol.io/)
- [MCP 服务器列表](https://github.com/modelcontextprotocol/servers)
- [MCP 规范](https://spec.modelcontextprotocol.io/)

---

**最后更新**: 2025-11-07
**版本**: Phase 3 Complete
