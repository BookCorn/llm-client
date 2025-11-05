# GPUI Chat Application - ChatGPT Style

## 概述

这是一个使用 GPUI 和 gpui-component 构建的现代化聊天应用，具有完整的 ChatGPT 风格界面和流式输出支持。

**最新状态**: ✅ 完整功能实现，支持真实 OpenAI API 和流式输出！

## 核心特性

### ✅ 已完成的功能

#### 1. **专业级 UI 体验**

- ✨ **ChatGPT 风格界面**：现代化的视觉设计
- 📝 **专业输入组件**：使用 gpui-component 的 `Input`，支持：
  - 中文输入法（IME）
  - 多行输入（Shift+Enter 换行，Enter 发送）
  - 文本选择、复制粘贴
  - Undo/Redo 支持
  - 最大高度滚动
- 💬 **消息气泡设计**：
  - 用户消息：蓝色气泡，右对齐
  - AI 回复：左对齐，支持 Markdown 渲染
- 📜 **可滚动消息区域**：完整的对话历史查看

#### 2. **真实 OpenAI API 集成** 🚀

- ✅ **流式响应**：实时逐字显示 AI 回复
- ✅ **推理模型支持**：
  - 自动检测推理摘要（Reasoning Summary）
  - 仅在 API 返回推理数据时显示"思考过程"
  - 支持 OpenAI o1、o3 等推理模型
- ✅ **推理过程展示**：
  - 实时计时器
  - 流式显示推理内容
  - 可展开/收起查看推理摘要
  - 自动保存推理时长
- ✅ **自定义端点**：支持配置 API Base URL
- ✅ **错误处理**：详细的错误提示和日志

#### 3. **Markdown 渲染** 📄

- 使用 `TextView::markdown` 渲染 AI 回复
- 支持标题、列表、代码块、粗体、斜体等
- 深色文字确保可读性

#### 4. **对话管理系统**

- 侧边栏显示所有对话
- 创建新对话（"+" 按钮）
- 切换对话
- 自动保存到本地存储（~/.gpui-chat/）
- UUID 标识每个对话

#### 5. **智能数据驱动架构** 🎯

- **按实际数据显示**：
  - 有推理摘要则显示，无则跳过
  - 符合 Raycast 的行为模式
- **为 MCP Server 做准备**：
  - 不预设或伪造任何数据
  - 完全基于 API 返回内容
  - 易于扩展到其他数据源

#### 6. **调试和诊断**

- 实时状态显示（API 模式、加载状态）
- 详细错误对话框
- 控制台日志输出

### 🎨 UI 细节

- **深色文字**：所有文字使用深色（0x1a1a1a）确保可读性
- **文本换行**：长文本自动换行，不会被截断
- **圆角按钮**：ChatGPT 风格的圆形发送按钮
- **响应式布局**：左侧边栏 + 主聊天区域
- **推理过程框**：
  - 浅灰色背景（0xf5f5f5）
  - 可展开/收起
  - 显示实时计时
  - 流式显示内容

## 技术栈

- **GPUI**: 0.2.x - 高性能 GPU 渲染的 UI 框架
- **gpui-component**: 专业组件库（Input、TextView、布局辅助）
- **async-openai**: 0.30.1 - OpenAI API 客户端，支持流式响应
- **tokio**: 1.48.0 - 异步运行时
- **serde/serde_json**: JSON 序列化
- **chrono**: 日期时间处理
- **uuid**: 唯一标识符
- **anyhow**: 错误处理

## 配置

### 环境变量

```bash
# 必需：OpenAI API 密钥
export OPENAI_API_KEY="your-api-key-here"

# 可选：自定义 API 端点（默认：OpenAI 官方）
export OPENAI_API_BASE="https://api.openai.com/v1"

# 可选：指定模型（默认：gpt-4）
export OPENAI_MODEL="gpt-4"
# 或使用推理模型以查看思考过程：
export OPENAI_MODEL="o1-preview"
```

### 配置示例

查看 `OPENAI_CONFIG.md` 获取详细配置说明。

## 快速开始

### 1. 安装依赖

```bash
# 确保已安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 配置环境变量

```bash
# 创建 .env 文件（可选）
echo 'OPENAI_API_KEY="your-api-key"' > .env
source .env
```

### 3. 构建和运行

```bash
# 构建项目
cargo build

# 运行应用
cargo run
```

### 4. 使用说明

- **发送消息**：在输入框输入文字，按 Enter 发送
- **多行输入**：按 Shift+Enter 换行
- **查看推理过程**：使用推理模型（如 o1）时自动显示
- **展开/收起推理**：点击推理摘要框标题
- **创建新对话**：点击左侧边栏的 "+" 按钮
- **切换对话**：点击左侧边栏的对话项

## 工作模式

### 真实 API 模式

当设置了 `OPENAI_API_KEY` 时：

- ✅ 调用真实 OpenAI API
- ✅ 流式显示 AI 回复
- ✅ 自动检测并显示推理摘要（如果有）
- ✅ 保存对话到本地

### 模拟模式

未设置 API Key 时：

- 🎭 显示模拟响应
- ✅ 流式输出演示
- ⚠️ 不显示推理摘要（模拟真实 API 行为）

## 文件结构

```
gpui-test/
├── src/
│   ├── main.rs                  # 主应用代码
│   ├── models/
│   │   ├── mod.rs              # 模型模块
│   │   ├── message.rs          # Message 结构（含推理字段）
│   │   └── conversation.rs     # Conversation 结构
│   ├── services/
│   │   ├── mod.rs              # 服务模块
│   │   ├── openai.rs           # OpenAI API 集成
│   │   └── storage.rs          # 本地存储服务
│   └── ui/
│       ├── mod.rs              # UI 模块
│       ├── message_view.rs     # 消息渲染（含推理展示）
│       └── sidebar.rs          # 侧边栏组件
├── Cargo.toml                   # 依赖配置
├── README.md                    # 本文件
├── OPENAI_CONFIG.md            # API 配置说明
└── QUICK_START.md              # 快速开始指南
```

对话数据存储在：

```
~/.gpui-chat/
├── {uuid-1}.json
├── {uuid-2}.json
└── ...
```

## 核心实现细节

### 流式输出架构

```rust
// 1. 后台线程调用 API
std::thread::spawn(move || {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        openai.get_streaming_completion(&messages, |chunk| {
            // 实时累积内容到 Arc<Mutex<String>>
            streaming_content_arc.push_str(&chunk);
        }).await
    });
});

// 2. UI 线程轮询更新
fn check_pending_response(&mut self) {
    if self.is_reasoning {
        let content = pending.streaming_content_arc.lock().unwrap();
        self.update_message_content(content);
        cx.notify(); // 触发重新渲染
    }
}
```

### 推理摘要处理

```rust
// API 返回 (content, reasoning_summary_opt)
if let Some(summary) = reasoning_summary_opt {
    // 只在有实际数据时显示
    self.reasoning_summary = Some(summary);
}

// UI 条件渲染
.when(self.is_reasoning && self.reasoning_summary.is_some(), |d| {
    // 显示推理过程框
})
```

### 数据模型

```rust
pub struct Message {
    pub role: String,                       // "user" or "assistant"
    pub content: String,                    // 消息内容
    pub timestamp: DateTime<Utc>,          // 时间戳
    pub reasoning_summary: Option<String>,  // 推理摘要
    pub reasoning_duration: Option<f64>,    // 推理时长（秒）
}
```

## 未来计划

### 即将实现

1. **MCP Server 集成** 🎯
   - 数据驱动架构已准备好
   - 可轻松接入 MCP 工具调用
   - 支持显示工具执行过程

2. **代码高亮**
   - 集成语法高亮库
   - 支持多种编程语言

3. **主题系统**
   - 深色/浅色模式切换
   - 自定义颜色方案

### 长期目标

4. **图片支持**
   - 图片上传
   - 图片渲染
   - 视觉模型集成

5. **高级功能**
   - Web Search
   - 文件上传
   - 导出对话
   - 快捷键系统

## 已知限制

1. **GPUI 学习曲线**：GPUI 框架较新，文档有限
2. **推理摘要**：当前 async-openai 0.30.1 需要自定义解析（等待官方支持）
3. **性能优化**：大量消息时的滚动性能待优化

## 技术亮点

### 成功解决的问题

1. **跨线程通信**
   - 使用 `Arc<Mutex<T>>` 和 `Arc<AtomicBool>` 实现
   - 后台 Tokio 运行时 + 主 UI 线程
   - 轮询机制确保实时更新

2. **流式 UI 更新**
   - 每次 `render()` 调用检查新内容
   - `cx.notify()` 触发重新渲染
   - 避免 UI 阻塞

3. **数据驱动设计**
   - 不预设任何假数据
   - 完全基于 API 返回
   - 易于扩展到新数据源

4. **专业输入组件**
   - gpui-component 的 `Input` 完美支持中文
   - 多行输入、文本选择、复制粘贴
   - 避免手工实现复杂的输入逻辑

## 调试技巧

### 查看日志

```bash
cargo run 2>&1 | grep -E "🧠|📝|✅|❌|📦"
```

### 测试 API 连接

```bash
bash test_custom_endpoint.sh
```

### 清空对话历史

```bash
rm -rf ~/.gpui-chat/*.json
```

## 贡献

欢迎贡献！请提交 Issue 或 Pull Request。

### 开发准则

- 保持代码简洁清晰
- 添加必要的注释
- 遵循 Rust 最佳实践
- 测试新功能

## 致谢

- **GPUI**: 感谢 Zed 团队创建的优秀 UI 框架
- **gpui-component**: Longbridge 的专业组件库
- **OpenAI**: 强大的 AI 模型
