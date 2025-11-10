/// 会话历史管理
///
/// 提供会话分叉、恢复和历史记录功能
use super::conversation::Conversation;
use super::message::Message;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 会话快照 - 某个时间点的会话状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSnapshot {
    /// 快照 ID
    pub id: Uuid,

    /// 原始会话 ID
    pub conversation_id: Uuid,

    /// 快照时的消息列表
    pub messages: Vec<Message>,

    /// 快照创建时间
    pub created_at: DateTime<Utc>,

    /// 快照描述
    pub description: String,

    /// 父快照 ID（用于跟踪分叉链）
    pub parent_snapshot_id: Option<Uuid>,
}

impl ConversationSnapshot {
    /// 从会话创建快照
    pub fn from_conversation(conversation: &Conversation, description: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id: conversation.id,
            messages: conversation.messages.clone(),
            created_at: Utc::now(),
            description,
            parent_snapshot_id: None,
        }
    }

    /// 创建子快照（分叉时使用）
    pub fn fork_from(
        parent: &ConversationSnapshot,
        messages: Vec<Message>,
        description: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id: parent.conversation_id,
            messages,
            created_at: Utc::now(),
            description,
            parent_snapshot_id: Some(parent.id),
        }
    }

    /// 恢复为会话
    pub fn restore_to_conversation(&self) -> Conversation {
        Conversation {
            id: self.conversation_id,
            title: format!("恢复自快照: {}", self.description),
            messages: self.messages.clone(),
            created_at: self.created_at,
        }
    }
}

/// 分叉点 - 记录可以分叉的位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkPoint {
    /// 分叉点 ID
    pub id: Uuid,

    /// 会话 ID
    pub conversation_id: Uuid,

    /// 分叉点在消息列表中的索引
    pub message_index: usize,

    /// 分叉点标记
    pub label: String,

    /// 创建时间
    pub created_at: DateTime<Utc>,
}

impl ForkPoint {
    pub fn new(conversation_id: Uuid, message_index: usize, label: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            conversation_id,
            message_index,
            label,
            created_at: Utc::now(),
        }
    }
}

/// 会话历史管理器
pub struct ConversationHistory {
    /// 所有快照
    snapshots: HashMap<Uuid, ConversationSnapshot>,

    /// 所有分叉点
    fork_points: HashMap<Uuid, ForkPoint>,

    /// 会话 ID 到快照 ID 的映射
    conversation_snapshots: HashMap<Uuid, Vec<Uuid>>,

    /// 自动快照启用标志
    auto_snapshot_enabled: bool,

    /// 自动快照间隔（消息数）
    auto_snapshot_interval: usize,
}

impl ConversationHistory {
    /// 创建新的历史管理器
    pub fn new() -> Self {
        Self {
            snapshots: HashMap::new(),
            fork_points: HashMap::new(),
            conversation_snapshots: HashMap::new(),
            auto_snapshot_enabled: true,
            auto_snapshot_interval: 10,
        }
    }

    /// 启用/禁用自动快照
    pub fn set_auto_snapshot(&mut self, enabled: bool, interval: usize) {
        self.auto_snapshot_enabled = enabled;
        self.auto_snapshot_interval = interval;
    }

    /// 创建快照
    pub fn create_snapshot(&mut self, conversation: &Conversation, description: String) -> Uuid {
        let snapshot = ConversationSnapshot::from_conversation(conversation, description);
        let snapshot_id = snapshot.id;

        // 保存快照
        self.snapshots.insert(snapshot_id, snapshot);

        // 更新会话到快照的映射
        self.conversation_snapshots
            .entry(conversation.id)
            .or_insert_with(Vec::new)
            .push(snapshot_id);

        snapshot_id
    }

    /// 获取快照
    pub fn get_snapshot(&self, snapshot_id: Uuid) -> Option<&ConversationSnapshot> {
        self.snapshots.get(&snapshot_id)
    }

    /// 列出会话的所有快照
    pub fn list_snapshots(&self, conversation_id: Uuid) -> Vec<&ConversationSnapshot> {
        if let Some(snapshot_ids) = self.conversation_snapshots.get(&conversation_id) {
            snapshot_ids
                .iter()
                .filter_map(|id| self.snapshots.get(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 从快照恢复会话
    pub fn restore_from_snapshot(&self, snapshot_id: Uuid) -> Result<Conversation> {
        let snapshot = self
            .snapshots
            .get(&snapshot_id)
            .ok_or_else(|| anyhow!("快照不存在: {}", snapshot_id))?;

        Ok(snapshot.restore_to_conversation())
    }

    /// 创建分叉点
    pub fn create_fork_point(
        &mut self,
        conversation_id: Uuid,
        message_index: usize,
        label: String,
    ) -> Uuid {
        let fork_point = ForkPoint::new(conversation_id, message_index, label);
        let fork_point_id = fork_point.id;

        self.fork_points.insert(fork_point_id, fork_point);
        fork_point_id
    }

    /// 从分叉点创建新会话
    pub fn fork_at_point(
        &mut self,
        conversation: &Conversation,
        fork_point_id: Uuid,
    ) -> Result<Conversation> {
        let fork_point = self
            .fork_points
            .get(&fork_point_id)
            .ok_or_else(|| anyhow!("分叉点不存在: {}", fork_point_id))?;

        if fork_point.conversation_id != conversation.id {
            return Err(anyhow!("分叉点不属于此会话"));
        }

        // 创建快照记录分叉前状态
        let parent_snapshot = ConversationSnapshot::from_conversation(
            conversation,
            format!("分叉前状态: {}", fork_point.label),
        );

        // 截取到分叉点的消息
        let forked_messages = conversation
            .messages
            .iter()
            .take(fork_point.message_index + 1)
            .cloned()
            .collect::<Vec<_>>();

        // 创建子快照
        let child_snapshot = ConversationSnapshot::fork_from(
            &parent_snapshot,
            forked_messages.clone(),
            format!("分叉自: {}", fork_point.label),
        );

        // 保存快照
        self.snapshots.insert(parent_snapshot.id, parent_snapshot);
        self.snapshots
            .insert(child_snapshot.id, child_snapshot.clone());

        // 创建新会话
        let mut new_conversation = Conversation {
            id: Uuid::new_v4(),
            title: format!("{} (分叉)", conversation.title),
            messages: forked_messages,
            created_at: Utc::now(),
        };

        // 更新标题
        if !new_conversation.messages.is_empty() {
            new_conversation.title = format!("{} - {}", conversation.title, fork_point.label);
        }

        Ok(new_conversation)
    }

    /// 从消息索引处分叉
    pub fn fork_at_message(
        &mut self,
        conversation: &Conversation,
        message_index: usize,
    ) -> Result<Conversation> {
        if message_index >= conversation.messages.len() {
            return Err(anyhow!("消息索引超出范围"));
        }

        // 创建临时分叉点
        let fork_point_id = self.create_fork_point(
            conversation.id,
            message_index,
            format!("消息 #{}", message_index + 1),
        );

        self.fork_at_point(conversation, fork_point_id)
    }

    /// 检查是否需要自动快照
    pub fn should_auto_snapshot(&self, conversation: &Conversation) -> bool {
        if !self.auto_snapshot_enabled {
            return false;
        }

        let message_count = conversation.messages.len();
        message_count > 0 && message_count % self.auto_snapshot_interval == 0
    }

    /// 自动创建快照（如果需要）
    pub fn auto_snapshot(&mut self, conversation: &Conversation) -> Option<Uuid> {
        if self.should_auto_snapshot(conversation) {
            Some(self.create_snapshot(
                conversation,
                format!("自动快照 - {} 条消息", conversation.messages.len()),
            ))
        } else {
            None
        }
    }

    /// 删除快照
    pub fn delete_snapshot(&mut self, snapshot_id: Uuid) -> Result<()> {
        let snapshot = self
            .snapshots
            .remove(&snapshot_id)
            .ok_or_else(|| anyhow!("快照不存在"))?;

        // 从映射中移除
        if let Some(snapshot_ids) = self
            .conversation_snapshots
            .get_mut(&snapshot.conversation_id)
        {
            snapshot_ids.retain(|id| *id != snapshot_id);
        }

        Ok(())
    }

    /// 删除分叉点
    pub fn delete_fork_point(&mut self, fork_point_id: Uuid) -> Result<()> {
        self.fork_points
            .remove(&fork_point_id)
            .ok_or_else(|| anyhow!("分叉点不存在"))?;
        Ok(())
    }

    /// 获取快照数量
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// 获取分叉点数量
    pub fn fork_point_count(&self) -> usize {
        self.fork_points.len()
    }

    /// 清理旧快照（保留最近 N 个）
    pub fn cleanup_old_snapshots(&mut self, conversation_id: Uuid, keep_count: usize) {
        if let Some(snapshot_ids) = self.conversation_snapshots.get_mut(&conversation_id) {
            // 按时间排序（新的在前）
            snapshot_ids.sort_by(|a, b| {
                let time_a = self.snapshots.get(a).map(|s| s.created_at);
                let time_b = self.snapshots.get(b).map(|s| s.created_at);
                time_b.cmp(&time_a)
            });

            // 删除多余的快照
            if snapshot_ids.len() > keep_count {
                let to_remove = snapshot_ids.split_off(keep_count);
                for id in to_remove {
                    self.snapshots.remove(&id);
                }
            }
        }
    }
}

impl Default for ConversationHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_conversation() -> Conversation {
        let mut conv = Conversation::new();
        conv.add_message(Message::new_user("Hello".to_string()));
        conv.add_message(Message::new_assistant("Hi there!".to_string()));
        conv.add_message(Message::new_user("How are you?".to_string()));
        conv
    }

    #[test]
    fn test_create_snapshot() {
        let mut history = ConversationHistory::new();
        let conv = create_test_conversation();

        let snapshot_id = history.create_snapshot(&conv, "Test snapshot".to_string());

        let snapshot = history.get_snapshot(snapshot_id).unwrap();
        assert_eq!(snapshot.messages.len(), 3);
        assert_eq!(snapshot.description, "Test snapshot");
    }

    #[test]
    fn test_restore_from_snapshot() {
        let mut history = ConversationHistory::new();
        let conv = create_test_conversation();

        let snapshot_id = history.create_snapshot(&conv, "Test".to_string());
        let restored = history.restore_from_snapshot(snapshot_id).unwrap();

        assert_eq!(restored.messages.len(), 3);
    }

    #[test]
    fn test_fork_at_message() {
        let mut history = ConversationHistory::new();
        let conv = create_test_conversation();

        // 从第二条消息分叉
        let forked = history.fork_at_message(&conv, 1).unwrap();

        assert_eq!(forked.messages.len(), 2);
        assert_ne!(forked.id, conv.id);
    }

    #[test]
    fn test_auto_snapshot() {
        let mut history = ConversationHistory::new();
        history.set_auto_snapshot(true, 2);

        let mut conv = Conversation::new();
        conv.add_message(Message::new_user("1".to_string()));

        // 不应该触发
        assert!(!history.should_auto_snapshot(&conv));

        conv.add_message(Message::new_user("2".to_string()));

        // 应该触发
        assert!(history.should_auto_snapshot(&conv));
    }

    #[test]
    fn test_list_snapshots() {
        let mut history = ConversationHistory::new();
        let conv = create_test_conversation();

        history.create_snapshot(&conv, "Snapshot 1".to_string());
        history.create_snapshot(&conv, "Snapshot 2".to_string());

        let snapshots = history.list_snapshots(conv.id);
        assert_eq!(snapshots.len(), 2);
    }

    #[test]
    fn test_cleanup_old_snapshots() {
        let mut history = ConversationHistory::new();
        let conv = create_test_conversation();

        // 创建 5 个快照
        for i in 0..5 {
            history.create_snapshot(&conv, format!("Snapshot {}", i));
        }

        assert_eq!(history.list_snapshots(conv.id).len(), 5);

        // 只保留 2 个
        history.cleanup_old_snapshots(conv.id, 2);

        assert_eq!(history.list_snapshots(conv.id).len(), 2);
    }
}
