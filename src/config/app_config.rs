/// 应用配置
///
/// 包含所有应用级别的配置
use super::provider_config::{Provider, ProviderType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 应用信息
    #[serde(default)]
    pub app: AppInfo,

    /// Provider 列表
    #[serde(default)]
    pub providers: Vec<Provider>,

    /// MCP 配置
    #[serde(default)]
    pub mcp: McpConfig,

    /// 会话配置
    #[serde(default)]
    pub conversation: ConversationConfig,

    /// UI 配置
    #[serde(default)]
    pub ui: UiConfig,

    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfig,
}

/// 应用信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    /// 应用名称
    #[serde(default = "default_app_name")]
    pub name: String,

    /// 版本
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_app_name() -> String {
    "GPUI Chat".to_string()
}

fn default_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

impl Default for AppInfo {
    fn default() -> Self {
        Self {
            name: default_app_name(),
            version: default_version(),
        }
    }
}

/// MCP 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// 是否启用 MCP
    #[serde(default = "default_false")]
    pub enabled: bool,

    /// MCP 服务器配置文件路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub servers_config: Option<String>,
}

fn default_false() -> bool {
    false
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            servers_config: None,
        }
    }
}

/// 会话配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationConfig {
    /// 自动快照
    #[serde(default)]
    pub auto_snapshot: AutoSnapshotConfig,

    /// 最大会话历史
    #[serde(default = "default_max_history")]
    pub max_history: usize,

    /// 保留的快照数量
    #[serde(default = "default_keep_snapshots")]
    pub keep_snapshots: usize,
}

fn default_max_history() -> usize {
    100
}

fn default_keep_snapshots() -> usize {
    20
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            auto_snapshot: AutoSnapshotConfig::default(),
            max_history: 100,
            keep_snapshots: 20,
        }
    }
}

/// 自动快照配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoSnapshotConfig {
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// 间隔（消息数）
    #[serde(default = "default_snapshot_interval")]
    pub interval: usize,
}

fn default_true() -> bool {
    true
}

fn default_snapshot_interval() -> usize {
    10
}

impl Default for AutoSnapshotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: 10,
        }
    }
}

/// UI 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// 主题
    #[serde(default = "default_theme")]
    pub theme: String,

    /// 字体大小
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// 窗口宽度
    #[serde(default = "default_window_width")]
    pub window_width: f64,

    /// 窗口高度
    #[serde(default = "default_window_height")]
    pub window_height: f64,
}

fn default_theme() -> String {
    "light".to_string()
}

fn default_font_size() -> f32 {
    14.0
}

fn default_window_width() -> f64 {
    1200.0
}

fn default_window_height() -> f64 {
    800.0
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "light".to_string(),
            font_size: 14.0,
            window_width: 1200.0,
            window_height: 800.0,
        }
    }
}

/// 存储配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// 数据目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,

    /// 是否加密 API Key
    #[serde(default = "default_false")]
    pub encrypt_api_keys: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            encrypt_api_keys: false,
        }
    }
}

impl AppConfig {
    /// 获取默认 provider
    pub fn default_provider(&self) -> Option<&Provider> {
        self.providers
            .iter()
            .find(|p| p.enabled)
            .or_else(|| self.providers.first())
    }

    /// 根据名称获取 provider
    pub fn get_provider(&self, name: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.name == name)
    }

    /// 获取所有启用的 providers
    pub fn enabled_providers(&self) -> Vec<&Provider> {
        self.providers.iter().filter(|p| p.enabled).collect()
    }

    /// 添加 provider
    pub fn add_provider(&mut self, provider: Provider) {
        self.providers.push(provider);
    }

    /// 移除 provider
    pub fn remove_provider(&mut self, name: &str) -> bool {
        let original_len = self.providers.len();
        self.providers.retain(|p| p.name != name);
        self.providers.len() < original_len
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), String> {
        if self.providers.is_empty() {
            return Err("至少需要配置一个 Provider".to_string());
        }

        // 检查是否有启用的 provider
        if !self.providers.iter().any(|p| p.enabled) {
            return Err("至少需要启用一个 Provider".to_string());
        }

        // 检查 provider 名称唯一性
        let mut names = std::collections::HashSet::new();
        for provider in &self.providers {
            if !names.insert(&provider.name) {
                return Err(format!("Provider 名称重复: {}", provider.name));
            }
        }

        // 检查 API Key
        for provider in &self.providers {
            if provider.enabled && provider.config.api_key.is_empty() {
                return Err(format!("Provider '{}' 的 API Key 不能为空", provider.name));
            }
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            app: AppInfo::default(),
            providers: vec![Provider::openai(
                "default".to_string(),
                "your-api-key-here".to_string(),
                "gpt-4".to_string(),
            )],
            mcp: McpConfig::default(),
            conversation: ConversationConfig::default(),
            ui: UiConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.app.name, "GPUI Chat");
        assert!(config.providers.len() > 0);
    }

    #[test]
    fn test_get_default_provider() {
        let config = AppConfig::default();
        let provider = config.default_provider().unwrap();
        assert_eq!(provider.name, "default");
    }

    #[test]
    fn test_get_provider_by_name() {
        let config = AppConfig::default();
        let provider = config.get_provider("default").unwrap();
        assert_eq!(provider.name, "default");
    }

    #[test]
    fn test_add_remove_provider() {
        let mut config = AppConfig::default();
        let initial_count = config.providers.len();

        config.add_provider(Provider::openai(
            "test".to_string(),
            "sk-test".to_string(),
            "gpt-3.5-turbo".to_string(),
        ));

        assert_eq!(config.providers.len(), initial_count + 1);

        let removed = config.remove_provider("test");
        assert!(removed);
        assert_eq!(config.providers.len(), initial_count);
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        assert!(config.validate().is_ok());

        // 清空 providers 应该失败
        config.providers.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_enabled_providers() {
        let mut config = AppConfig::default();
        config.providers[0].enabled = false;

        config.add_provider(Provider::openai(
            "enabled".to_string(),
            "sk-test".to_string(),
            "gpt-4".to_string(),
        ));

        let enabled = config.enabled_providers();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "enabled");
    }
}
