/// 统一配置管理
///
/// 使用 TOML 文件替代环境变量，提供集中的配置管理
pub mod app_config;
pub mod provider_config;

pub use app_config::AppConfig;
pub use provider_config::{Provider, ProviderConfig, ProviderType};

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 配置文件加载优先级
///
/// 1. 命令行指定的路径
/// 2. ./config.toml（当前目录）
/// 3. ~/.config/gpui-test/config.toml（用户配置目录）
/// 4. 默认配置
pub struct ConfigLoader;

impl ConfigLoader {
    /// 从文件加载配置
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<AppConfig> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("无法读取配置文件: {}", e))?;

        toml::from_str(&content).map_err(|e| anyhow!("无法解析配置文件: {}", e))
    }

    /// 尝试从默认位置加载配置
    pub fn load_default() -> Result<AppConfig> {
        // 优先级 1: 当前目录
        let current_dir = PathBuf::from("./config.toml");
        if current_dir.exists() {
            return Self::load_from_file(current_dir);
        }

        // 优先级 2: 用户配置目录
        if let Some(config_dir) = dirs::config_dir() {
            let user_config = config_dir.join("gpui-test").join("config.toml");
            if user_config.exists() {
                return Self::load_from_file(user_config);
            }
        }

        // 使用默认配置
        Ok(AppConfig::default())
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(config: &AppConfig, path: P) -> Result<()> {
        let content =
            toml::to_string_pretty(config).map_err(|e| anyhow!("无法序列化配置: {}", e))?;

        std::fs::write(path.as_ref(), content).map_err(|e| anyhow!("无法写入配置文件: {}", e))?;

        Ok(())
    }

    /// 获取默认配置目录
    pub fn default_config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("gpui-test"))
    }

    /// 确保配置目录存在
    pub fn ensure_config_dir() -> Result<PathBuf> {
        let config_dir = Self::default_config_dir().ok_or_else(|| anyhow!("无法确定配置目录"))?;

        std::fs::create_dir_all(&config_dir).map_err(|e| anyhow!("无法创建配置目录: {}", e))?;

        Ok(config_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(config.providers.len() > 0);
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("providers"));
    }

    #[test]
    fn test_config_deserialization() {
        let toml_str = r#"
            [app]
            name = "GPUI Chat"
            version = "0.1.0"

            [[providers]]
            name = "openai"
            type = "openai"
            enabled = true

            [providers.config]
            api_key = "sk-test"
            model = "gpt-4"
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].name, "openai");
    }
}
