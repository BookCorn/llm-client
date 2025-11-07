/// 内置工具
///
/// 提供开箱即用的常用工具

pub mod shell;

pub use shell::ShellTool;

use std::sync::Arc;

use super::registry::ToolRegistry;

/// 注册所有内置工具
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    let _ = registry.register(Arc::new(ShellTool::new()));
    // 未来可以添加更多内置工具：
    // let _ = registry.register(Arc::new(WebSearchTool::new()));
    // let _ = registry.register(Arc::new(FileReadTool::new()));
}
