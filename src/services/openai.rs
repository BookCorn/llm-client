use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
    },
    Client,
};

use crate::models::Message;

#[derive(Clone)]
pub struct OpenAIService {
    client: Client<OpenAIConfig>,
}

impl OpenAIService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn get_completion(&self, messages: &[Message]) -> Result<String> {
        // Convert our Message type to OpenAI's ChatCompletionRequestMessage
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

        // Create chat completion request
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4")
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
}

impl Default for OpenAIService {
    fn default() -> Self {
        Self::new()
    }
}
