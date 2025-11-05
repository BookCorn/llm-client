use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: DateTime<Utc>,
    // 推理摘要相关字段
    #[serde(default)]
    pub reasoning_summary: Option<String>,
    #[serde(default)]
    pub reasoning_duration: Option<f64>,
}

impl Message {
    pub fn new_user(content: String) -> Self {
        Self {
            role: "user".to_string(),
            content,
            timestamp: Utc::now(),
            reasoning_summary: None,
            reasoning_duration: None,
        }
    }

    pub fn new_assistant(content: String) -> Self {
        Self {
            role: "assistant".to_string(),
            content,
            timestamp: Utc::now(),
            reasoning_summary: None,
            reasoning_duration: None,
        }
    }

    pub fn is_user(&self) -> bool {
        self.role == "user"
    }
}
