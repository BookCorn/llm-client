use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::message::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub messages: Vec<Message>,
    pub created_at: DateTime<Utc>,
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: "New Conversation".to_string(),
            messages: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        // Update title if this is the first message
        if self.messages.is_empty() && message.is_user() {
            self.title = self.generate_title_from_content(&message.content);
        }
        self.messages.push(message);
    }

    fn generate_title_from_content(&self, content: &str) -> String {
        content
            .lines()
            .next()
            .unwrap_or("New Conversation")
            .chars()
            .take(50)
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}
