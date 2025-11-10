# OpenAI Configuration Guide

本文档说明如何配置OpenAI服务以使用自定义endpoint。

## 环境变量配置

### 基本使用（官方OpenAI API）

```bash
export OPENAI_API_KEY="your-api-key-here"
cargo run
```

### 使用自定义Endpoint

```bash
# 设置API密钥
export OPENAI_API_KEY="your-api-key-here"

# 设置自定义endpoint（例如Azure OpenAI或本地部署的模型）
export OPENAI_API_BASE="https://your-custom-endpoint.com/v1"

# 可选：指定使用的模型
export OPENAI_MODEL="gpt-3.5-turbo"

cargo run
```

## 支持的环境变量

| 环境变量 | 必需 | 默认值 | 说明 |
|---------|------|--------|------|
| `OPENAI_API_KEY` | ✅ | - | OpenAI API密钥 |
| `OPENAI_API_BASE` | ❌ | OpenAI官方endpoint | 自定义API endpoint URL |
| `OPENAI_MODEL` | ❌ | `gpt-4` | 使用的模型名称 |

## 常见使用场景

### 1. Azure OpenAI Service

```bash
export OPENAI_API_KEY="your-azure-api-key"
export OPENAI_API_BASE="https://your-resource-name.openai.azure.com/openai/deployments/your-deployment-name"
export OPENAI_MODEL="gpt-4"
```

### 2. 本地部署的LLM (如使用LocalAI)

```bash
export OPENAI_API_KEY="not-needed"  # 某些本地服务可能不需要
export OPENAI_API_BASE="http://localhost:8080/v1"
export OPENAI_MODEL="gpt-3.5-turbo"  # 或您的本地模型名称
```

### 3. OpenAI兼容服务 (如vLLM, Text Generation Inference等)

```bash
export OPENAI_API_KEY="any-string-works"
export OPENAI_API_BASE="http://your-server:8000/v1"
export OPENAI_MODEL="your-model-name"
```

### 4. 代理服务器

```bash
export OPENAI_API_KEY="your-api-key"
export OPENAI_API_BASE="https://your-proxy.com/v1"
export OPENAI_MODEL="gpt-4"
```

## 代码中使用

### 使用环境变量（推荐）

```rust
use gpui_test::services::OpenAIService;

// 自动从环境变量读取配置
let service = OpenAIService::new();
```

### 手动配置

```rust
use gpui_test::services::{OpenAIService, OpenAIServiceConfig};

// 创建自定义配置
let config = OpenAIServiceConfig::new(
    "your-api-key".to_string(),
    Some("https://api.custom-endpoint.com/v1".to_string()),
    "gpt-3.5-turbo".to_string()
);

// 使用自定义配置创建服务
let service = OpenAIService::with_config(config);
```

### 从环境变量显式创建配置

```rust
use gpui_test::services::{OpenAIService, OpenAIServiceConfig};

let config = OpenAIServiceConfig::from_env()?;
let service = OpenAIService::with_config(config);
```

## 检查当前配置

```rust
// 获取当前使用的模型
let model = service.model();
println!("Using model: {}", model);

// 获取API endpoint
if let Some(base) = service.api_base() {
    println!("Using custom endpoint: {}", base);
} else {
    println!("Using official OpenAI endpoint");
}

// 检查是否使用自定义endpoint
if service.is_custom_endpoint() {
    println!("Custom endpoint is configured");
}
```

## 故障排除

### API调用失败

1. **检查API密钥是否正确**
   ```bash
   echo $OPENAI_API_KEY
   ```

2. **检查endpoint URL格式**
   - 确保URL以`http://`或`https://`开头
   - 通常以`/v1`结尾
   - 不要包含尾部斜杠

3. **验证endpoint可访问性**
   ```bash
   curl -H "Authorization: Bearer $OPENAI_API_KEY" $OPENAI_API_BASE/models
   ```

### 模型不存在错误

确保`OPENAI_MODEL`设置为您的endpoint支持的模型：

```bash
# 列出可用模型
curl -H "Authorization: Bearer $OPENAI_API_KEY" $OPENAI_API_BASE/models
```

## 安全建议

1. ✅ 不要在代码中硬编码API密钥
2. ✅ 使用环境变量或安全的配置管理系统
3. ✅ 在生产环境使用HTTPS endpoint
4. ✅ 定期轮换API密钥
5. ❌ 不要将包含密钥的配置文件提交到版本控制

## 示例：完整的配置脚本

创建一个`.env`文件（不要提交到git）：

```bash
# .env
OPENAI_API_KEY=sk-your-key-here
OPENAI_API_BASE=https://your-endpoint.com/v1
OPENAI_MODEL=gpt-4
```

然后使用以下方式加载：

```bash
# 加载环境变量
source .env

# 运行应用
cargo run
```

或者使用`dotenv` crate在代码中加载。
