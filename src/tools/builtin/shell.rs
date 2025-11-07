/// Shell 工具
///
/// 允许执行 shell 命令
///
/// ⚠️ 安全警告：此工具可以执行任意命令，具有高风险
/// 建议：
/// 1. 默认需要用户审批
/// 2. 在生产环境中使用沙箱
/// 3. 限制可执行的命令白名单
///
/// 参考 codex-rs/core/src/tools/handlers/shell.rs

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::process::Command;

use crate::tools::spec::{Tool, ToolOutput, ToolSpec};

/// Shell 工具
pub struct ShellTool {
    /// 是否需要审批（默认：true）
    requires_approval: bool,
}

impl ShellTool {
    /// 创建 Shell 工具
    pub fn new() -> Self {
        Self {
            requires_approval: true, // 默认需要审批，因为 Shell 命令有风险
        }
    }

    /// 创建无需审批的 Shell 工具（⚠️ 危险！仅用于测试）
    #[allow(dead_code)]
    pub fn new_auto_approve() -> Self {
        Self {
            requires_approval: false,
        }
    }

    /// 执行 shell 命令（内部实现）
    fn execute_command(command: &str) -> Result<String> {
        // 使用 sh -c 执行命令（兼容 Unix 系统）
        #[cfg(unix)]
        let output = Command::new("sh").arg("-c").arg(command).output()?;

        // 使用 cmd /C 执行命令（Windows 系统）
        #[cfg(windows)]
        let output = Command::new("cmd").arg("/C").arg(command).output()?;

        // 检查退出状态
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Command failed with exit code {}: {}",
                output.status.code().unwrap_or(-1),
                stderr
            ));
        }

        // 返回标准输出
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.to_string())
    }
}

impl Default for ShellTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::function(
            "shell".to_string(),
            "Execute a shell command and return the output. Use this to interact with the system, run programs, or gather information.".to_string(),
            json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute (e.g., 'ls -la', 'git status', 'python script.py')"
                    }
                },
                "required": ["command"]
            }),
        )
    }

    async fn execute(&self, args: Value) -> Result<ToolOutput> {
        // 提取命令参数
        let command = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' parameter"))?;

        println!("🔧 执行 Shell 命令: {}", command);

        // 执行命令
        match Self::execute_command(command) {
            Ok(output) => {
                println!("✅ 命令执行成功，输出 {} 字节", output.len());
                Ok(ToolOutput::text(output))
            }
            Err(e) => {
                println!("❌ 命令执行失败: {}", e);
                Ok(ToolOutput::error(format!("{}", e)))
            }
        }
    }

    fn requires_approval(&self) -> bool {
        self.requires_approval
    }

    fn approval_prompt(&self, args: &Value) -> String {
        let command = args["command"].as_str().unwrap_or("<unknown>");
        format!(
            "🔐 Shell 工具需要审批\n\n\
            命令: {}\n\n\
            ⚠️ 此命令将在您的系统上执行，可能会修改文件或系统设置。\n\
            是否允许执行？",
            command
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shell_tool_echo() {
        let tool = ShellTool::new_auto_approve();
        let args = json!({"command": "echo 'hello world'"});

        let result = tool.execute(args).await.unwrap();
        match result {
            ToolOutput::Text(text) => {
                assert!(text.contains("hello world"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_shell_tool_pwd() {
        let tool = ShellTool::new_auto_approve();
        let args = json!({"command": "pwd"});

        let result = tool.execute(args).await.unwrap();
        match result {
            ToolOutput::Text(text) => {
                assert!(!text.is_empty());
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_shell_tool_invalid_command() {
        let tool = ShellTool::new_auto_approve();
        let args = json!({"command": "nonexistent_command_12345"});

        let result = tool.execute(args).await.unwrap();
        match result {
            ToolOutput::Error(msg) => {
                assert!(msg.contains("failed") || msg.contains("not found"));
            }
            _ => panic!("Expected error output"),
        }
    }

    #[test]
    fn test_shell_tool_spec() {
        let tool = ShellTool::new();
        let spec = tool.spec();

        assert_eq!(spec.name, "shell");
        assert_eq!(spec.tool_type, "function");
        assert!(spec.parameters["properties"]["command"].is_object());
    }

    #[test]
    fn test_shell_tool_requires_approval() {
        let tool_with_approval = ShellTool::new();
        assert!(tool_with_approval.requires_approval());

        let tool_without_approval = ShellTool::new_auto_approve();
        assert!(!tool_without_approval.requires_approval());
    }
}
