/// MCP 配置加载
///
/// 从 TOML 文件或环境变量加载 MCP 服务器配置

use super::types::McpServerConfig;
use anyhow::{Context, Result};
use std::path::Path;

/// MCP 配置文件结构
#[derive(Debug, Clone, serde::Deserialize)]
pub struct McpConfig {
    /// MCP 服务器列表
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

impl McpConfig {
    /// 从 TOML 文件加载配置
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read MCP config file: {}", path.display()))?;

        let config: McpConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse MCP config file: {}", path.display()))?;

        Ok(config)
    }

    /// 从环境变量加载配置
    ///
    /// 查找 MCP_SERVERS_CONFIG 环境变量指定的文件路径
    pub fn from_env() -> Result<Option<Self>> {
        if let Ok(config_path) = std::env::var("MCP_SERVERS_CONFIG") {
            println!("📂 从环境变量加载 MCP 配置: {}", config_path);
            Ok(Some(Self::from_file(config_path)?))
        } else {
            Ok(None)
        }
    }

    /// 从默认位置加载配置
    ///
    /// 尝试以下位置（按优先级）：
    /// 1. 环境变量 MCP_SERVERS_CONFIG
    /// 2. ./mcp_servers.toml
    /// 3. ~/.config/gpui-test/mcp_servers.toml
    pub fn load_default() -> Result<Option<Self>> {
        // 1. 尝试环境变量
        if let Some(config) = Self::from_env()? {
            return Ok(Some(config));
        }

        // 2. 尝试当前目录
        let local_path = Path::new("mcp_servers.toml");
        if local_path.exists() {
            println!("📂 从当前目录加载 MCP 配置: {}", local_path.display());
            return Ok(Some(Self::from_file(local_path)?));
        }

        // 3. 尝试用户配置目录
        if let Some(config_dir) = dirs::config_dir() {
            let user_path = config_dir.join("gpui-test").join("mcp_servers.toml");
            if user_path.exists() {
                println!("📂 从配置目录加载 MCP 配置: {}", user_path.display());
                return Ok(Some(Self::from_file(user_path)?));
            }
        }

        println!("ℹ️  未找到 MCP 配置文件，跳过 MCP 集成");
        Ok(None)
    }

    /// 获取已启用的服务器配置
    pub fn enabled_servers(&self) -> Vec<&McpServerConfig> {
        self.servers.iter().filter(|s| s.enabled).collect()
    }

    /// 获取服务器数量
    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    /// 获取已启用的服务器数量
    pub fn enabled_count(&self) -> usize {
        self.enabled_servers().len()
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            servers: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_toml_config() {
        let toml_content = r#"
[[servers]]
name = "filesystem"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "mcp-server-filesystem"
args = []

[servers.tool_filter]
allow = ["read_file", "write_file"]
deny = []
"#;

        let config: McpConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "filesystem");
        assert!(config.servers[0].enabled);
    }

    #[test]
    fn test_load_from_file() {
        // 创建临时配置文件
        let temp_file = std::env::temp_dir().join("test_mcp_config.toml");
        {
            let mut file = std::fs::File::create(&temp_file).unwrap();
            file.write_all(
                br#"
[[servers]]
name = "test-server"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "echo"
args = []
"#,
            )
            .unwrap();
        }

        let config = McpConfig::from_file(&temp_file).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "test-server");

        // 清理
        std::fs::remove_file(temp_file).ok();
    }

    #[test]
    fn test_enabled_servers() {
        let toml_content = r#"
[[servers]]
name = "server1"
enabled = true

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "echo"
args = []

[[servers]]
name = "server2"
enabled = false

[servers.connection_type]
type = "stdio"

[servers.connection_params]
command = "echo"
args = []
"#;

        let config: McpConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.server_count(), 2);
        assert_eq!(config.enabled_count(), 1);

        let enabled = config.enabled_servers();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "server1");
    }
}
