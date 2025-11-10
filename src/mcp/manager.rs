/// MCP 连接管理器
///
/// 管理多个 MCP 服务器连接，自动发现和注册工具
use super::connection::{ConnectionStatus, McpConnection};
use super::stdio::StdioConnection;
use super::tool_adapter::McpToolAdapter;
use super::types::{ConnectionParams, ConnectionType, McpServerConfig};
use crate::tools::registry::ToolRegistry;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// MCP 连接管理器
pub struct McpConnectionManager {
    /// 所有连接（server_name -> connection）
    pub(crate) connections: HashMap<String, Arc<Mutex<dyn McpConnection>>>,

    /// 服务器配置
    pub(crate) configs: Vec<McpServerConfig>,
}

impl McpConnectionManager {
    /// 创建新的连接管理器
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
            configs: Vec::new(),
        }
    }

    /// 从配置创建
    pub fn from_configs(configs: Vec<McpServerConfig>) -> Self {
        Self {
            connections: HashMap::new(),
            configs,
        }
    }

    /// 添加服务器配置
    pub fn add_config(&mut self, config: McpServerConfig) {
        self.configs.push(config);
    }

    /// 连接到所有已启用的服务器
    pub async fn connect_all(&mut self) -> Result<()> {
        println!("🔌 开始连接到 MCP 服务器...");

        // 克隆配置以避免借用冲突
        let configs = self.configs.clone();

        for config in configs.iter() {
            if !config.enabled {
                println!("⏭️  跳过已禁用的服务器: {}", config.name);
                continue;
            }

            match self.connect_server(config).await {
                Ok(_) => println!("✅ 连接到 MCP 服务器: {}", config.name),
                Err(e) => {
                    println!("❌ 连接到 {} 失败: {}", config.name, e);
                    // 继续连接其他服务器
                }
            }
        }

        Ok(())
    }

    /// 连接到单个服务器
    async fn connect_server(&mut self, config: &McpServerConfig) -> Result<()> {
        let connection_arc = self.create_connection(config)?;

        {
            let mut conn = connection_arc.lock().await;
            conn.connect().await?;
        }

        self.connections.insert(config.name.clone(), connection_arc);

        Ok(())
    }

    /// 根据配置创建连接
    pub(crate) fn create_connection(
        &self,
        config: &McpServerConfig,
    ) -> Result<Arc<Mutex<dyn McpConnection>>> {
        match &config.connection_type {
            ConnectionType::Stdio => {
                if let ConnectionParams::Stdio { command, args, env } = &config.connection_params {
                    let conn = StdioConnection::new(
                        config.name.clone(),
                        command.clone(),
                        args.clone(),
                        env.clone(),
                    );
                    Ok(Arc::new(Mutex::new(conn)))
                } else {
                    Err(anyhow!("Invalid connection params for stdio connection"))
                }
            }
            ConnectionType::Http => {
                if let ConnectionParams::Http { url, headers } = &config.connection_params {
                    let conn = super::http::HttpConnection::new(
                        config.name.clone(),
                        url.clone(),
                        headers.clone(),
                    )?;
                    Ok(Arc::new(Mutex::new(conn)))
                } else {
                    Err(anyhow!("Invalid connection params for HTTP connection"))
                }
            }
        }
    }

    /// 发现并注册所有工具
    pub async fn discover_and_register_tools(&mut self, registry: &mut ToolRegistry) -> Result<()> {
        println!("🔍 开始发现 MCP 工具...");

        let mut total_tools = 0;

        for (server_name, connection) in &self.connections {
            match self
                .discover_tools_from_server(server_name, connection.clone(), registry)
                .await
            {
                Ok(count) => {
                    println!("✅ 从 {} 发现 {} 个工具", server_name, count);
                    total_tools += count;
                }
                Err(e) => {
                    println!("❌ 从 {} 发现工具失败: {}", server_name, e);
                }
            }
        }

        println!("🎉 总共发现 {} 个 MCP 工具", total_tools);
        Ok(())
    }

    /// 从单个服务器发现工具
    async fn discover_tools_from_server(
        &self,
        server_name: &str,
        connection: Arc<Mutex<dyn McpConnection>>,
        registry: &mut ToolRegistry,
    ) -> Result<usize> {
        // 获取服务器配置
        let config = self
            .configs
            .iter()
            .find(|c| c.name == server_name)
            .ok_or_else(|| anyhow!("Server config not found: {}", server_name))?;

        // 列出工具
        let tools = {
            let mut conn = connection.lock().await;
            conn.list_tools().await?
        };

        let mut registered_count = 0;

        for mcp_tool in tools {
            // 应用工具过滤器
            if !config.tool_filter.is_allowed(&mcp_tool.name) {
                println!("⏭️  跳过被过滤的工具: {}", mcp_tool.name);
                continue;
            }

            // 创建工具实例
            let tool = McpToolAdapter::create_tool(
                server_name.to_string(),
                mcp_tool.clone(),
                connection.clone(),
            );

            // 注册到 registry
            match registry.register(tool) {
                Ok(_) => {
                    registered_count += 1;
                }
                Err(e) => {
                    println!("⚠️  注册工具 {} 失败: {}", mcp_tool.name, e);
                }
            }
        }

        Ok(registered_count)
    }

    /// 断开所有连接
    pub async fn disconnect_all(&mut self) -> Result<()> {
        println!("🔌 断开所有 MCP 服务器连接...");

        for (name, connection) in &mut self.connections {
            let mut conn = connection.lock().await;
            match conn.disconnect().await {
                Ok(_) => println!("✅ 断开连接: {}", name),
                Err(e) => println!("⚠️  断开 {} 时出错: {}", name, e),
            }
        }

        self.connections.clear();
        Ok(())
    }

    /// 获取连接状态
    pub async fn get_status(&self) -> HashMap<String, ConnectionStatus> {
        let mut status_map = HashMap::new();

        for (name, connection) in &self.connections {
            let conn = connection.lock().await;
            status_map.insert(name.clone(), conn.status());
        }

        status_map
    }

    /// 获取连接数量
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }
}

impl Default for McpConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let manager = McpConnectionManager::new();
        assert_eq!(manager.connection_count(), 0);
    }

    #[test]
    fn test_add_config() {
        let mut manager = McpConnectionManager::new();

        let config = McpServerConfig {
            name: "test".to_string(),
            connection_type: ConnectionType::Stdio,
            connection_params: ConnectionParams::Stdio {
                command: "echo".to_string(),
                args: vec![],
                env: HashMap::new(),
            },
            tool_filter: Default::default(),
            enabled: true,
        };

        manager.add_config(config);
        assert_eq!(manager.configs.len(), 1);
    }
}
