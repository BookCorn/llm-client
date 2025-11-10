/// 工具注册表
///
/// 管理所有可用工具，提供查找和列举功能
use std::collections::HashMap;
use std::sync::Arc;

use super::spec::{Tool, ToolSpec};

/// 工具注册表
///
/// 线程安全的工具管理器
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// 创建空的工具注册表
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// 注册工具
    ///
    /// # 参数
    /// - `tool`: 实现 Tool trait 的工具实例
    ///
    /// # 返回
    /// - `Ok(())`: 注册成功
    /// - `Err(String)`: 工具名称已存在
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), String> {
        let name = tool.name().to_string();

        if self.tools.contains_key(&name) {
            return Err(format!("Tool '{}' already registered", name));
        }

        self.tools.insert(name, tool);
        Ok(())
    }

    /// 查找工具
    ///
    /// # 参数
    /// - `name`: 工具名称
    ///
    /// # 返回
    /// - `Some(Arc<dyn Tool>)`: 找到工具
    /// - `None`: 工具不存在
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// 列举所有工具
    pub fn list(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.values().cloned().collect()
    }

    /// 获取所有工具规范（用于生成 API 请求）
    pub fn specs(&self) -> Vec<ToolSpec> {
        self.tools.values().map(|tool| tool.spec()).collect()
    }

    /// 工具数量
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// 检查工具是否存在
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// 移除工具
    pub fn remove(&mut self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.remove(name)
    }

    /// 清空所有工具
    pub fn clear(&mut self) {
        self.tools.clear();
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::spec::{Tool, ToolOutput, ToolSpec};
    use async_trait::async_trait;
    use serde_json::Value;

    struct MockTool {
        name: String,
    }

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            &self.name
        }

        fn spec(&self) -> ToolSpec {
            ToolSpec::function(
                self.name.clone(),
                "Mock tool".to_string(),
                serde_json::json!({}),
            )
        }

        async fn execute(&self, _args: Value) -> anyhow::Result<ToolOutput> {
            Ok(ToolOutput::text("mock"))
        }
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool {
            name: "test".to_string(),
        });

        assert!(registry.register(tool).is_ok());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_duplicate() {
        let mut registry = ToolRegistry::new();
        let tool1 = Arc::new(MockTool {
            name: "test".to_string(),
        });
        let tool2 = Arc::new(MockTool {
            name: "test".to_string(),
        });

        assert!(registry.register(tool1).is_ok());
        assert!(registry.register(tool2).is_err());
    }

    #[test]
    fn test_registry_get() {
        let mut registry = ToolRegistry::new();
        let tool = Arc::new(MockTool {
            name: "test".to_string(),
        });

        registry.register(tool).unwrap();
        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_specs() {
        let mut registry = ToolRegistry::new();
        let tool1 = Arc::new(MockTool {
            name: "tool1".to_string(),
        });
        let tool2 = Arc::new(MockTool {
            name: "tool2".to_string(),
        });

        registry.register(tool1).unwrap();
        registry.register(tool2).unwrap();

        let specs = registry.specs();
        assert_eq!(specs.len(), 2);
    }
}
