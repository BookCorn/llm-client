/// MCP (Model Context Protocol) 集成模块
///
/// 此模块实现了 MCP 客户端功能，支持：
/// - 多种连接方式（stdio, HTTP/SSE）
/// - 动态工具发现
/// - 工具限定名机制
/// - 配置管理
pub mod config;
pub mod connection;
pub mod http;
pub mod manager;
pub mod stdio;
pub mod tool_adapter;
pub mod types;

pub use config::McpConfig;
pub use connection::{ConnectionStatus, McpConnection};
pub use http::HttpConnection;
pub use manager::McpConnectionManager;
pub use stdio::StdioConnection;
pub use tool_adapter::McpToolAdapter;
pub use types::{McpServerConfig, McpToolDefinition};
