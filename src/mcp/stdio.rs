/// stdio 进程连接实现
///
/// 通过 stdin/stdout 与 MCP 服务器进程通信

use super::connection::{ConnectionStatus, McpConnection};
use super::types::{McpRequest, McpResponse};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

/// stdio 连接
pub struct StdioConnection {
    /// 服务器名称
    server_name: String,

    /// 命令
    command: String,

    /// 参数
    args: Vec<String>,

    /// 环境变量
    env: HashMap<String, String>,

    /// 连接状态
    status: ConnectionStatus,

    /// 子进程（仅在连接时存在）
    process: Option<Arc<Mutex<StdioProcess>>>,

    /// 请求 ID 计数器
    request_id: AtomicU64,
}

/// stdio 进程封装
struct StdioProcess {
    /// 子进程
    _child: Child,

    /// stdin
    stdin: ChildStdin,

    /// stdout reader
    stdout: BufReader<ChildStdout>,
}

impl StdioConnection {
    /// 创建新的 stdio 连接
    pub fn new(
        server_name: String,
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    ) -> Self {
        Self {
            server_name,
            command,
            args,
            env,
            status: ConnectionStatus::Disconnected,
            process: None,
            request_id: AtomicU64::new(1),
        }
    }

    /// 启动子进程
    fn spawn_process(&self) -> Result<StdioProcess> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.envs(&self.env);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null()); // TODO: 捕获 stderr 用于日志

        let mut child = cmd.spawn().map_err(|e| {
            anyhow!("Failed to spawn MCP server '{}': {}", self.server_name, e)
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            anyhow!("Failed to open stdin for MCP server '{}'", self.server_name)
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            anyhow!("Failed to open stdout for MCP server '{}'", self.server_name)
        })?;

        Ok(StdioProcess {
            _child: child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }
}

#[async_trait]
impl McpConnection for StdioConnection {
    fn server_name(&self) -> &str {
        &self.server_name
    }

    fn status(&self) -> ConnectionStatus {
        self.status
    }

    async fn connect(&mut self) -> Result<()> {
        if self.status == ConnectionStatus::Connected {
            return Ok(());
        }

        self.status = ConnectionStatus::Connecting;

        let process = self.spawn_process()?;
        self.process = Some(Arc::new(Mutex::new(process)));
        self.status = ConnectionStatus::Connected;

        println!("✅ 已连接到 MCP 服务器: {}", self.server_name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(process) = self.process.take() {
            // Drop the process (will kill the child)
            drop(process);
        }

        self.status = ConnectionStatus::Disconnected;
        println!("🔌 已断开 MCP 服务器: {}", self.server_name);
        Ok(())
    }

    async fn send_request(&mut self, request: McpRequest) -> Result<McpResponse> {
        let process = self.process.as_ref().ok_or_else(|| {
            anyhow!("Not connected to MCP server '{}'", self.server_name)
        })?;

        // 序列化请求
        let request_json = serde_json::to_string(&request)?;
        let request_line = format!("{}\n", request_json);

        // 发送请求到 stdin
        {
            let mut proc = process.lock().await;
            proc.stdin.write_all(request_line.as_bytes()).await?;
            proc.stdin.flush().await?;
        }

        // 读取响应从 stdout
        let response_line = {
            let mut proc = process.lock().await;
            let mut line = String::new();
            proc.stdout.read_line(&mut line).await?;
            line
        };

        // 解析响应
        let response: McpResponse = serde_json::from_str(&response_line).map_err(|e| {
            anyhow!(
                "Failed to parse MCP response from '{}': {} (response: {})",
                self.server_name,
                e,
                response_line.trim()
            )
        })?;

        Ok(response)
    }

    fn next_request_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}

impl Drop for StdioConnection {
    fn drop(&mut self) {
        // 确保子进程被清理
        if let Some(process) = self.process.take() {
            drop(process);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_connection_creation() {
        let conn = StdioConnection::new(
            "test-server".to_string(),
            "echo".to_string(),
            vec![],
            HashMap::new(),
        );

        assert_eq!(conn.server_name(), "test-server");
        assert_eq!(conn.status(), ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_request_id_increment() {
        let conn = StdioConnection::new(
            "test".to_string(),
            "echo".to_string(),
            vec![],
            HashMap::new(),
        );

        assert_eq!(conn.next_request_id(), 1);
        assert_eq!(conn.next_request_id(), 2);
        assert_eq!(conn.next_request_id(), 3);
    }
}
