use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};
use futures::StreamExt;

use crate::models::Message;

/// Configuration for OpenAI API
#[derive(Clone, Debug)]
pub struct OpenAIServiceConfig {
    pub api_key: String,
    pub api_base: Option<String>,
    pub model: String,
}

impl OpenAIServiceConfig {
    /// Create config from environment variables
    ///
    /// Environment variables:
    /// - OPENAI_API_KEY (required): Your OpenAI API key
    /// - OPENAI_API_BASE (optional): Custom API endpoint (e.g., "https://api.openai.com/v1" or your own endpoint)
    /// - OPENAI_MODEL (optional): Model to use (default: "gpt-4")
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY environment variable not set"))?;

        let api_base = std::env::var("OPENAI_API_BASE").ok();
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4".to_string());

        Ok(Self {
            api_key,
            api_base,
            model,
        })
    }

    /// Create config with custom values
    #[allow(dead_code)]
    pub fn new(api_key: String, api_base: Option<String>, model: String) -> Self {
        Self {
            api_key,
            api_base,
            model,
        }
    }
}

#[derive(Clone)]
pub struct OpenAIService {
    client: Client<OpenAIConfig>,
    config: OpenAIServiceConfig,
}

impl OpenAIService {
    /// Create a new OpenAI service with default configuration (from env vars)
    pub fn new() -> Self {
        let config = OpenAIServiceConfig::from_env().unwrap_or_else(|_| {
            // Fallback to default config if env vars not set
            OpenAIServiceConfig {
                api_key: String::new(),
                api_base: None,
                model: "gpt-4".to_string(),
            }
        });

        Self::with_config(config)
    }

    /// Create a new OpenAI service with custom configuration
    pub fn with_config(config: OpenAIServiceConfig) -> Self {
        let mut openai_config = OpenAIConfig::new().with_api_key(&config.api_key);

        // Set custom API base if provided
        if let Some(api_base) = &config.api_base {
            openai_config = openai_config.with_api_base(api_base);
        }

        let client = Client::with_config(openai_config);

        Self { client, config }
    }

    /// Get the current model being used
    #[allow(dead_code)]
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Get the API base URL if set
    #[allow(dead_code)]
    pub fn api_base(&self) -> Option<&str> {
        self.config.api_base.as_deref()
    }

    /// Check if using custom endpoint
    #[allow(dead_code)]
    pub fn is_custom_endpoint(&self) -> bool {
        self.config.api_base.is_some()
    }

    pub async fn get_completion(&self, messages: &[Message]) -> Result<String> {
        let openai_messages = self.convert_messages(messages)?;

        // Create chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(openai_messages)
            .build()?;

        // Make API call
        let response = self.client.chat().create(request).await?;

        // Extract the response content
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_else(|| "No response generated.".to_string());

        Ok(content)
    }

    #[allow(dead_code)]
    pub async fn get_streaming_completion<F>(&self, messages: &[Message], mut on_chunk: F) -> Result<(String, Option<String>)>
    where
        F: FnMut(String) + Send,
    {
        let openai_messages = self.convert_messages(messages)?;

        // Create streaming chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(openai_messages)
            .build()?;

        // Make streaming API call
        let mut stream = self.client.chat().create_stream(request).await?;

        let mut full_response = String::new();
        // 注意：标准的 OpenAI API 没有单独的推理字段
        // 对于 o1 模型，可能需要特殊处理
        // 这里我们返回 None，表示没有推理摘要

        // Process the stream
        while let Some(result) = stream.next().await {
            match result {
                Ok(response) => {
                    for choice in response.choices {
                        if let Some(content) = &choice.delta.content {
                            full_response.push_str(content);
                            on_chunk(content.clone());
                        }
                        // TODO: 未来如果 API 返回推理信息，在这里提取
                        // 例如：choice.reasoning_content 或其他字段
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }

        // 返回 (内容, 推理摘要)
        // 当前 API 没有推理摘要，所以返回 None
        Ok((full_response, None))
    }

    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<ChatCompletionRequestMessage>> {
        let mut openai_messages: Vec<ChatCompletionRequestMessage> = Vec::new();

        // Add system message
        openai_messages.push(
            ChatCompletionRequestSystemMessageArgs::default()
                .content("You are a helpful AI assistant. You can use Markdown formatting in your responses.")
                .build()?
                .into(),
        );

        // Add conversation history
        for msg in messages {
            if msg.is_user() {
                openai_messages.push(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(msg.content.clone())
                        .build()?
                        .into(),
                );
            } else {
                // For assistant messages, we need to use a different type
                openai_messages.push(
                    ChatCompletionRequestMessage::Assistant(
                        async_openai::types::ChatCompletionRequestAssistantMessageArgs::default()
                            .content(msg.content.clone())
                            .build()?
                    )
                );
            }
        }

        Ok(openai_messages)
    }
}

impl Default for OpenAIService {
    fn default() -> Self {
        Self::new()
    }
}
