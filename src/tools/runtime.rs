/// 工具执行运行时
///
/// 负责工具的实际执行，包括审批机制、错误处理、超时控制
///
/// 参考 codex-rs/core/src/tools/runtimes/

use anyhow::{anyhow, Result};
use std::sync::Arc;
use std::time::Duration;

use super::registry::ToolRegistry;
use super::spec::{ToolInvocation, ToolOutput, ToolResult};

/// 审批策略
#[derive(Clone, Debug, PartialEq)]
pub enum ApprovalPolicy {
    /// 自动批准所有工具
    AutoApprove,
    /// 自动批准安全工具，危险工具需要审批
    AutoApproveSafe,
    /// 所有工具都需要审批
    RequireApproval,
}

/// 执行结果
#[derive(Clone, Debug)]
pub enum ExecutionResult {
    /// 执行成功
    Success(ToolResult),
    /// 等待用户审批
    AwaitingApproval {
        invocation: ToolInvocation,
        prompt: String,
    },
    /// 执行失败
    Error {
        call_id: String,
        error: String,
    },
}

/// 工具运行时配置
#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// 审批策略
    pub approval_policy: ApprovalPolicy,
    /// 执行超时（毫秒）
    pub timeout_ms: u64,
    /// 是否启用沙箱（暂未实现）
    pub sandboxed: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            approval_policy: ApprovalPolicy::AutoApproveSafe,
            timeout_ms: 60000, // 60秒
            sandboxed: false,
        }
    }
}

/// 工具执行运行时
///
/// 参考 codex-rs/core/src/tools/runtimes/
pub struct ToolRuntime {
    registry: Arc<ToolRegistry>,
    config: RuntimeConfig,
}

impl ToolRuntime {
    /// 创建运行时
    pub fn new(registry: Arc<ToolRegistry>, config: RuntimeConfig) -> Self {
        Self { registry, config }
    }

    /// 执行工具调用
    ///
    /// # 参数
    /// - `invocation`: 工具调用请求
    ///
    /// # 返回
    /// - `ExecutionResult`: 执行结果（成功、等待审批、失败）
    ///
    /// # 参考
    /// - codex-rs/core/src/tools/runtimes/
    /// - 文档第200-203行
    pub async fn execute(&self, invocation: ToolInvocation) -> ExecutionResult {
        // 查找工具
        let tool = match self.registry.get(&invocation.name) {
            Some(t) => t,
            None => {
                return ExecutionResult::Error {
                    call_id: invocation.call_id.clone(),
                    error: format!("Tool '{}' not found", invocation.name),
                };
            }
        };

        // 检查是否需要审批
        if self.requires_approval(&*tool) {
            let prompt = tool.approval_prompt(&invocation.arguments);
            return ExecutionResult::AwaitingApproval { invocation, prompt };
        }

        // 执行工具（带超时）
        self.execute_with_timeout(invocation, tool).await
    }

    /// 带超时执行工具
    async fn execute_with_timeout(
        &self,
        invocation: ToolInvocation,
        tool: Arc<dyn super::spec::Tool>,
    ) -> ExecutionResult {
        let timeout = Duration::from_millis(self.config.timeout_ms);
        let call_id = invocation.call_id.clone();

        let result = tokio::time::timeout(timeout, tool.execute(invocation.arguments)).await;

        match result {
            Ok(Ok(output)) => ExecutionResult::Success(ToolResult::new(call_id, output)),
            Ok(Err(e)) => ExecutionResult::Error {
                call_id,
                error: format!("{:#}", e),
            },
            Err(_) => ExecutionResult::Error {
                call_id,
                error: format!("Tool execution timeout ({}ms)", self.config.timeout_ms),
            },
        }
    }

    /// 强制执行（跳过审批）
    ///
    /// 用于用户批准后执行
    pub async fn execute_approved(&self, invocation: ToolInvocation) -> ExecutionResult {
        let tool = match self.registry.get(&invocation.name) {
            Some(t) => t,
            None => {
                return ExecutionResult::Error {
                    call_id: invocation.call_id.clone(),
                    error: format!("Tool '{}' not found", invocation.name),
                };
            }
        };

        self.execute_with_timeout(invocation, tool).await
    }

    /// 拒绝执行
    ///
    /// 用户拒绝审批时调用
    pub fn reject(&self, invocation: ToolInvocation) -> ExecutionResult {
        ExecutionResult::Error {
            call_id: invocation.call_id,
            error: "User rejected tool execution".to_string(),
        }
    }

    /// 检查是否需要审批
    fn requires_approval(&self, tool: &dyn super::spec::Tool) -> bool {
        match self.config.approval_policy {
            ApprovalPolicy::AutoApprove => false,
            ApprovalPolicy::RequireApproval => true,
            ApprovalPolicy::AutoApproveSafe => {
                // 工具自行声明是否需要审批
                tool.requires_approval()
            }
        }
    }
}

/// 批量执行工具调用（并行）
///
/// # 参考
/// - codex-rs/core/src/tools/parallel.rs
/// - 文档第69行
pub async fn execute_parallel(
    runtime: Arc<ToolRuntime>,
    invocations: Vec<ToolInvocation>,
) -> Vec<ExecutionResult> {
    let mut handles = vec![];

    for invocation in invocations {
        let runtime = runtime.clone();
        let handle = tokio::spawn(async move { runtime.execute(invocation).await });
        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => {
                results.push(ExecutionResult::Error {
                    call_id: "unknown".to_string(),
                    error: format!("Task join error: {}", e),
                });
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::spec::{Tool, ToolSpec};
    use async_trait::async_trait;
    use serde_json::Value;

    struct MockTool {
        requires_approval: bool,
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock"
        }

        fn spec(&self) -> ToolSpec {
            ToolSpec::function("mock".to_string(), "Mock tool".to_string(), serde_json::json!({}))
        }

        async fn execute(&self, _args: Value) -> Result<ToolOutput> {
            Ok(ToolOutput::text("mock output"))
        }

        fn requires_approval(&self) -> bool {
            self.requires_approval
        }
    }

    #[tokio::test]
    async fn test_execute_auto_approve() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(MockTool {
                requires_approval: false,
            }))
            .unwrap();

        let runtime = ToolRuntime::new(
            Arc::new(registry),
            RuntimeConfig {
                approval_policy: ApprovalPolicy::AutoApprove,
                ..Default::default()
            },
        );

        let invocation = ToolInvocation::new(
            "call_1".to_string(),
            "mock".to_string(),
            serde_json::json!({}),
        );

        let result = runtime.execute(invocation).await;
        assert!(matches!(result, ExecutionResult::Success(_)));
    }

    #[tokio::test]
    async fn test_execute_require_approval() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(MockTool {
                requires_approval: true,
            }))
            .unwrap();

        let runtime = ToolRuntime::new(
            Arc::new(registry),
            RuntimeConfig {
                approval_policy: ApprovalPolicy::AutoApproveSafe,
                ..Default::default()
            },
        );

        let invocation = ToolInvocation::new(
            "call_1".to_string(),
            "mock".to_string(),
            serde_json::json!({}),
        );

        let result = runtime.execute(invocation).await;
        assert!(matches!(result, ExecutionResult::AwaitingApproval { .. }));
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let registry = ToolRegistry::new();
        let runtime = ToolRuntime::new(Arc::new(registry), RuntimeConfig::default());

        let invocation = ToolInvocation::new(
            "call_1".to_string(),
            "nonexistent".to_string(),
            serde_json::json!({}),
        );

        let result = runtime.execute(invocation).await;
        assert!(matches!(result, ExecutionResult::Error { .. }));
    }
}
