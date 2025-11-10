/// Provider 配置
///
/// 支持多个 AI Provider（OpenAI, Azure OpenAI, Anthropic 等）
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Provider 类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    OpenAI,
    AzureOpenAI,
    Anthropic,
    Custom,
}

/// Provider 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Provider 名称（用户自定义）
    pub name: String,

    /// Provider 类型
    #[serde(rename = "type")]
    pub provider_type: ProviderType,

    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Provider 特定配置
    pub config: ProviderConfig,
}

fn default_true() -> bool {
    true
}

/// Provider 特定配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API Key（本地存储，后续加密）
    pub api_key: String,

    /// API Base URL（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    /// 默认模型
    pub model: String,

    /// Responses API 配置
    #[serde(default)]
    pub responses_api: ResponsesApiConfig,

    /// 工具配置
    #[serde(default)]
    pub tools: ToolsConfig,

    /// 重试配置
    #[serde(default)]
    pub retry: RetryConfig,

    /// 超时配置
    #[serde(default)]
    pub timeout: TimeoutConfig,

    /// 遥测配置
    #[serde(default)]
    pub telemetry: TelemetryConfig,

    /// 生成配置
    #[serde(default)]
    pub generation: GenerationConfig,

    /// 额外配置（特定 provider 使用）
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Responses API 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponsesApiConfig {
    /// 是否启用 Responses API
    #[serde(default = "default_false")]
    pub enabled: bool,

    /// Reasoning effort
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,

    /// Reasoning summary
    #[serde(default = "default_reasoning_summary")]
    pub reasoning_summary: String,
}

fn default_false() -> bool {
    false
}

fn default_reasoning_effort() -> String {
    "medium".to_string()
}

fn default_reasoning_summary() -> String {
    "auto".to_string()
}

impl Default for ResponsesApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            reasoning_effort: "medium".to_string(),
            reasoning_summary: "auto".to_string(),
        }
    }
}

/// 工具配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// 是否启用工具
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 审批策略
    #[serde(default = "default_approval_policy")]
    pub approval_policy: String,

    /// 最大工具轮数
    #[serde(default = "default_max_tool_rounds")]
    pub max_tool_rounds: usize,
}

fn default_approval_policy() -> String {
    "safe".to_string()
}

fn default_max_tool_rounds() -> usize {
    5
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            approval_policy: "safe".to_string(),
            max_tool_rounds: 5,
        }
    }
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// 最大重试次数
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// 基础延迟（毫秒）
    #[serde(default = "default_base_delay_ms")]
    pub base_delay_ms: u64,

    /// 最大延迟（毫秒）
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,

    /// 抖动（毫秒）
    #[serde(default = "default_jitter_ms")]
    pub jitter_ms: u64,
}

fn default_max_attempts() -> u32 {
    3
}

fn default_base_delay_ms() -> u64 {
    1000
}

fn default_max_delay_ms() -> u64 {
    60000
}

fn default_jitter_ms() -> u64 {
    500
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 1000,
            max_delay_ms: 60000,
            jitter_ms: 500,
        }
    }
}

/// 超时配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutConfig {
    /// 请求超时（秒）
    #[serde(default = "default_request_timeout")]
    pub request_timeout_secs: u64,

    /// 流式空闲超时（秒）
    #[serde(default = "default_stream_idle_timeout")]
    pub stream_idle_timeout_secs: u64,
}

fn default_request_timeout() -> u64 {
    120
}

fn default_stream_idle_timeout() -> u64 {
    30
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            request_timeout_secs: 120,
            stream_idle_timeout_secs: 30,
        }
    }
}

impl TimeoutConfig {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }

    pub fn stream_idle_timeout(&self) -> Duration {
        Duration::from_secs(self.stream_idle_timeout_secs)
    }
}

/// 遥测配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// 是否启用遥测
    #[serde(default = "default_false")]
    pub enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

/// 生成配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    /// 默认预设
    #[serde(default = "default_preset")]
    pub preset: String,

    /// Temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Top P
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Frequency Penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,

    /// Presence Penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,

    /// Max Tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

fn default_preset() -> String {
    "balanced".to_string()
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            preset: "balanced".to_string(),
            temperature: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            max_tokens: None,
        }
    }
}

impl ProviderConfig {
    /// 转换为 OpenAIServiceConfig
    pub fn to_openai_service_config(&self) -> crate::services::openai::OpenAIServiceConfig {
        use crate::services::openai::{OpenAIServiceConfig, RetryPolicyConfig, TimeoutConfig};
        use crate::tools::ApprovalPolicy;

        let approval_policy = match self.tools.approval_policy.as_str() {
            "auto" => ApprovalPolicy::AutoApprove,
            "safe" => ApprovalPolicy::AutoApproveSafe,
            "require" => ApprovalPolicy::RequireApproval,
            _ => ApprovalPolicy::AutoApproveSafe,
        };

        OpenAIServiceConfig {
            api_key: self.api_key.clone(),
            api_base: self.api_base.clone(),
            model: self.model.clone(),
            use_responses_api: self.responses_api.enabled,
            reasoning_effort: self.responses_api.reasoning_effort.clone(),
            reasoning_summary: self.responses_api.reasoning_summary.clone(),
            enable_tools: self.tools.enabled,
            tool_approval_policy: approval_policy,
            retry_policy: RetryPolicyConfig {
                max_attempts: self.retry.max_attempts,
                base_delay_ms: self.retry.base_delay_ms,
                max_delay_ms: self.retry.max_delay_ms,
                jitter_ms: self.retry.jitter_ms,
            },
            timeout_config: TimeoutConfig {
                request_timeout: self.timeout.request_timeout(),
                stream_idle_timeout: self.timeout.stream_idle_timeout(),
            },
            telemetry_enabled: self.telemetry.enabled,
        }
    }
}

impl Provider {
    /// 创建 OpenAI provider
    pub fn openai(name: String, api_key: String, model: String) -> Self {
        Self {
            name,
            provider_type: ProviderType::OpenAI,
            enabled: true,
            config: ProviderConfig {
                api_key,
                api_base: None,
                model,
                responses_api: ResponsesApiConfig::default(),
                tools: ToolsConfig::default(),
                retry: RetryConfig::default(),
                timeout: TimeoutConfig::default(),
                telemetry: TelemetryConfig::default(),
                generation: GenerationConfig::default(),
                extra: HashMap::new(),
            },
        }
    }

    /// 创建 Azure OpenAI provider
    pub fn azure_openai(name: String, api_key: String, api_base: String, model: String) -> Self {
        Self {
            name,
            provider_type: ProviderType::AzureOpenAI,
            enabled: true,
            config: ProviderConfig {
                api_key,
                api_base: Some(api_base),
                model,
                responses_api: ResponsesApiConfig::default(),
                tools: ToolsConfig::default(),
                retry: RetryConfig::default(),
                timeout: TimeoutConfig::default(),
                telemetry: TelemetryConfig::default(),
                generation: GenerationConfig::default(),
                extra: HashMap::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = Provider::openai(
            "my-openai".to_string(),
            "sk-test".to_string(),
            "gpt-4".to_string(),
        );

        assert_eq!(provider.name, "my-openai");
        assert_eq!(provider.provider_type, ProviderType::OpenAI);
        assert!(provider.enabled);
    }

    #[test]
    fn test_provider_serialization() {
        let provider = Provider::openai(
            "test".to_string(),
            "sk-123".to_string(),
            "gpt-4".to_string(),
        );

        let toml_str = toml::to_string_pretty(&provider).unwrap();
        assert!(toml_str.contains("name = \"test\""));
        assert!(toml_str.contains("type = \"openai\""));
    }

    #[test]
    fn test_config_defaults() {
        let config = ProviderConfig {
            api_key: "test".to_string(),
            api_base: None,
            model: "gpt-4".to_string(),
            responses_api: ResponsesApiConfig::default(),
            tools: ToolsConfig::default(),
            retry: RetryConfig::default(),
            timeout: TimeoutConfig::default(),
            telemetry: TelemetryConfig::default(),
            generation: GenerationConfig::default(),
            extra: HashMap::new(),
        };

        assert_eq!(config.retry.max_attempts, 3);
        assert_eq!(config.timeout.request_timeout_secs, 120);
        assert!(config.tools.enabled);
    }
}
