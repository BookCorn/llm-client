# 快速开始指南

## 安装和运行

### 1. 克隆和构建

```bash
git clone <repository-url>
cd gpui-test
cargo build
```

### 2. 配置API

#### 选项A: 使用官方OpenAI API

```bash
export OPENAI_API_KEY="sk-your-api-key-here"
cargo run
```

#### 选项B: 使用自定义Endpoint

```bash
# 例如：本地LLM
export OPENAI_API_KEY="not-needed"
export OPENAI_API_BASE="http://localhost:8080/v1"
export OPENAI_MODEL="llama-2-7b"
cargo run
```

#### 选项C: 使用.env文件

```bash
# 复制示例配置
cp .env.example .env

# 编辑.env文件填入您的配置
nano .env

# 加载并运行
source .env && cargo run
```

### 3. Mock模式（不需要API key）

如果不设置`OPENAI_API_KEY`，应用会以mock模式运行，返回模拟响应。

```bash
cargo run
```

## 功能特性

### ✅ 已实现功能

- 📝 专业输入框（基于gpui-component的Input组件）
- 🌏 **完美支持中文输入**（IME输入法）
- 🎨 **ChatGPT风格UI**（白色输入框、绿色圆形发送按钮、优雅的"思考"提示）
- 📊 Markdown渲染（支持代码块、列表、格式化等）
- 💬 多轮对话管理
- 💾 自动保存到本地（`~/.gpui-chat/`）
- 📜 消息滚动
- ➕ 创建新对话
- 🔄 切换不同对话
- 🤖 异步API调用（不阻塞UI）
- 🔌 支持自定义API endpoint

### 🎯 使用技巧

#### 键盘快捷键

- `Enter` - 发送消息
- 支持完整的文本输入功能：
  - 光标导航、选择、复制、粘贴
  - 中文输入法（IME）完美支持
  - 输入框右侧有清除按钮

#### 对话管理

- 点击左侧 `+` 按钮创建新对话
- 点击已有对话切换
- 对话自动保存到 `~/.gpui-chat/`

#### UI特性

- **ChatGPT风格界面**：
  - 白色输入框，带阴影和圆角边框
  - 绿色圆形发送按钮（显示 ↑ 图标）
  - 优雅的"ChatGPT 正在思考..."提示
  - 输入框有内容且未在加载时，发送按钮才会激活

## 支持的Endpoint

### 官方服务

- ✅ OpenAI (官方)
- ✅ Azure OpenAI
- ✅ OpenRouter
- ✅ Together.ai
- ✅ Anyscale Endpoints

### 本地部署

- ✅ LocalAI
- ✅ vLLM
- ✅ Ollama (通过litellm)
- ✅ LM Studio
- ✅ Text Generation Inference
- ✅ 任何OpenAI兼容的API

## 配置示例

### Azure OpenAI

```bash
export OPENAI_API_KEY="your-azure-api-key"
export OPENAI_API_BASE="https://your-resource.openai.azure.com/openai/deployments/gpt-4"
export OPENAI_MODEL="gpt-4"
```

### LocalAI (本地)

```bash
# 启动LocalAI
docker run -p 8080:8080 localai/localai

# 配置应用
export OPENAI_API_KEY="not-needed"
export OPENAI_API_BASE="http://localhost:8080/v1"
export OPENAI_MODEL="gpt-3.5-turbo"  # 或您下载的模型名称
```

### vLLM (本地)

```bash
# 启动vLLM
python -m vllm.entrypoints.openai.api_server \
    --model meta-llama/Llama-2-7b-chat-hf \
    --port 8000

# 配置应用
export OPENAI_API_KEY="EMPTY"
export OPENAI_API_BASE="http://localhost:8000/v1"
export OPENAI_MODEL="meta-llama/Llama-2-7b-chat-hf"
```

### Ollama (通过litellm)

```bash
# 安装并启动Ollama
ollama serve

# 安装litellm
pip install litellm

# 启动litellm proxy
litellm --model ollama/llama2

# 配置应用
export OPENAI_API_KEY="ollama"
export OPENAI_API_BASE="http://localhost:11434/v1"
export OPENAI_MODEL="llama2"
```

## 故障排除

### 问题：API调用失败

**检查项**:
1. API key是否正确
2. Endpoint URL格式是否正确（应以`/v1`结尾）
3. 网络连接是否正常
4. 如果使用本地服务，确保服务已启动

```bash
# 测试endpoint连接
curl -H "Authorization: Bearer $OPENAI_API_KEY" \
     $OPENAI_API_BASE/models
```

### 问题：模型不存在

确保`OPENAI_MODEL`设置为您的服务支持的模型：

```bash
# 列出可用模型
curl -H "Authorization: Bearer $OPENAI_API_KEY" \
     $OPENAI_API_BASE/models
```

### 问题：UI显示问题

1. 确保使用最新版本
2. 尝试清除对话历史：`rm -rf ~/.gpui-chat/`
3. 重新构建：`cargo clean && cargo build`

## 开发

### 项目结构

```
src/
├── main.rs              # 主应用逻辑
├── models/              # 数据模型
│   ├── message.rs
│   └── conversation.rs
├── services/            # 业务逻辑
│   ├── openai.rs       # OpenAI API服务
│   └── storage.rs      # 本地存储
└── ui/                  # UI组件
    ├── sidebar.rs
    └── message_view.rs
```

### 添加新功能

详见各模块的代码注释和`OPENAI_CONFIG.md`。

## 更多文档

- [OpenAI配置详解](OPENAI_CONFIG.md) - 详细的配置说明
- [测试脚本](test_custom_endpoint.sh) - 自动化测试工具

## 许可证

[添加您的许可证信息]

## 贡献

欢迎提交Issue和Pull Request！
