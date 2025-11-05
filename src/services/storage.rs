use anyhow::Result;
use std::fs;
use std::path::PathBuf;

use crate::models::Conversation;

#[derive(Clone)]
pub struct StorageService {
    storage_path: PathBuf,
}

impl StorageService {
    pub fn new() -> Result<Self> {
        let storage_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gpui-chat");

        if !storage_path.exists() {
            fs::create_dir_all(&storage_path)?;
        }

        Ok(Self { storage_path })
    }

    pub fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        if conversation.is_empty() {
            return Ok(());
        }

        let file_path = self.storage_path.join(format!("{}.json", conversation.id));
        let json = serde_json::to_string_pretty(conversation)?;
        fs::write(file_path, json)?;
        Ok(())
    }

    pub fn load_conversations(&self) -> Result<Vec<Conversation>> {
        let mut conversations = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.storage_path) {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(conv) = serde_json::from_str::<Conversation>(&content) {
                        conversations.push(conv);
                    }
                }
            }
        }

        // Sort by creation date (newest first)
        conversations.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(conversations)
    }

    #[allow(dead_code)]
    pub fn delete_conversation(&self, conversation_id: uuid::Uuid) -> Result<()> {
        let file_path = self.storage_path.join(format!("{}.json", conversation_id));
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }
}

impl Default for StorageService {
    fn default() -> Self {
        Self::new().expect("Failed to create storage service")
    }
}
