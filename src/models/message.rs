use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    pub fn new_user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
            timestamp: Utc::now(),
        }
    }

    pub fn new_assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            timestamp: Utc::now(),
        }
    }

    pub fn is_user(&self) -> bool {
        self.role == "user"
    }
}
