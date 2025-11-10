/// 工具调用系统
///
/// 提供统一的工具注册、路由和执行框架
/// 支持本地工具和 MCP 工具
///
/// 参考：
/// - codex-rs/core/src/tools/spec.rs (工具定义)
/// - codex-rs/core/src/tools/router.rs (工具路由)
/// - codex-rs/core/src/tools/runtimes/ (工具执行)
pub mod registry;
pub mod router;
pub mod runtime;
pub mod spec;

// 内置工具
pub mod builtin;

pub use registry::ToolRegistry;
pub use router::ToolRouter;
pub use runtime::{ApprovalPolicy, ExecutionResult, RuntimeConfig, ToolRuntime};
pub use spec::{Tool, ToolOutput, ToolSpec};
