/// 完成结果类型
///
/// 扩展了简单的字符串返回，支持工具调用信息
use crate::tools::spec::ToolInvocation;

/// Completion 结果
#[derive(Clone, Debug)]
pub struct CompletionResult {
    /// 助手回复内容
    pub content: String,

    /// 推理摘要（如果有）
    pub reasoning_summary: Option<String>,

    /// 检测到的工具调用
    pub tool_calls: Vec<ToolInvocation>,
}

impl CompletionResult {
    /// 创建新的结果
    pub fn new(
        content: String,
        reasoning_summary: Option<String>,
        tool_calls: Vec<ToolInvocation>,
    ) -> Self {
        Self {
            content,
            reasoning_summary,
            tool_calls,
        }
    }

    /// 创建无工具调用的简单结果
    pub fn simple(content: String, reasoning_summary: Option<String>) -> Self {
        Self {
            content,
            reasoning_summary,
            tool_calls: Vec::new(),
        }
    }

    /// 是否有工具调用
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}
