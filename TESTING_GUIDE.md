# 测试指南 - Markdown 渲染与推理摘要

## 已修复的问题

### 1. ✅ 推理摘要实时渲染
**问题**: 推理摘要只在API完成后才显示，不能实时更新
**修复**: 在流式传输过程中实时更新消息的 `reasoning_summary` 字段

### 2. ✅ 详细调试输出
**新增**:
- 显示每个SSE事件的完整数据
- 显示推理摘要和内容的实时预览
- 显示API返回的原始delta内容

## 测试步骤

### 准备工作

1. **设置环境变量**（使用 Responses API）:
```bash
export OPENAI_API_KEY="your-key"
export OPENAI_API_BASE="https://api.ncpsnetworks.com/v1"  # 或你的API地址
export OPENAI_MODEL="gpt-5-mini"
export OPENAI_USE_RESPONSES_API=true  # 重要！启用 Responses API
```

2. **运行应用**:
```bash
cargo run
```

### 测试 1: 推理摘要实时显示

**目标**: 验证推理摘要在流式传输时实时更新

**测试步骤**:
1. 在输入框输入一个需要推理的问题，例如：
   ```
   请用概率论解释贝叶斯定理，并给出一个实际例子
   ```

2. **观察控制台输出** - 应该看到：
   ```
   🧠 推理摘要delta: "I need to explain..."
   🔄 实时更新推理摘要: 45 字符
   🧠 更新推理摘要预览: I need to explain Bayes' theorem...
   ```

3. **观察UI** - 应该看到：
   - 推理过程框（🧠 推理过程）出现
   - 点击展开后，内容**实时增加**（不是等全部完成才显示）
   - 推理内容流式显示

**预期结果**: ✅ 推理摘要实时流式显示，不需要等待完成

---

### 测试 2: Markdown 格式渲染

**目标**: 验证API返回的内容是否包含Markdown格式，以及是否正确渲染

**测试步骤**:
1. 在输入框输入：
   ```
   请给我一个包含多种Markdown格式的回复示例，包括：
   1. **加粗文字**
   2. *斜体文字*
   3. # 一级标题
   4. ## 二级标题
   5. ### 三级标题
   6. 代码块
   7. 列表
   ```

2. **观察控制台输出** - 查找这些关键信息：
   ```
   📦 输出delta: "# Title\n\nThis is **bold**"
   📝 更新内容预览: # Markdown Example

   Here's a **bold** text and *italic* text...
   ```

3. **分析输出内容**:
   - ✅ 如果看到 `**` 和 `*` 符号 → API返回了Markdown格式
   - ✅ 如果看到 `#` 符号 → API返回了标题格式
   - ❌ 如果只看到纯文本 → API没有返回Markdown格式

4. **观察UI** - 应该看到：
   - 标题显示为更大的字体
   - **加粗文字** 显示为粗体
   - *斜体文字* 显示为斜体
   - 代码块有背景色

**预期结果**:
- ✅ 控制台显示包含Markdown符号的内容
- ✅ UI正确渲染Markdown格式

---

### 测试 3: 调试API返回格式

**目标**: 如果Markdown没有正确显示，查看API实际返回了什么

**测试步骤**:
1. 发送任意消息

2. **查看控制台的详细输出**:
   ```
   ℹ️ 事件: response.output_text.delta | 数据: {"delta":"..."}
   📦 输出delta: "actual content here"
   ```

3. **检查delta内容**:
   - 查看 `📦 输出delta:` 后面显示的实际内容
   - 确认是否包含 Markdown 标记（`**`, `*`, `#`, 等）

4. **如果没有Markdown标记**:
   - 可能原因1: API提供商的模型不返回Markdown格式
   - 可能原因2: 需要在prompt中明确要求Markdown格式
   - 解决方案: 在系统提示中添加Markdown要求

---

## 可能遇到的问题

### 问题 1: 没有看到推理摘要

**症状**:
```
📦 输出delta: "content..."
✅ 响应完成 | 完整响应长度: 200 | 推理长度: 0
```

**原因**:
- API使用的是Chat Completions API，不是Responses API
- 或者模型不是推理模型（不是 gpt-5-mini, o1, o3 等）

**解决方案**:
```bash
# 确认使用 Responses API
export OPENAI_USE_RESPONSES_API=true

# 确认使用推理模型
export OPENAI_MODEL="gpt-5-mini"  # 或 o1-mini, o3-mini

# 重新运行
cargo run
```

### 问题 2: 看到推理摘要事件但是空的

**症状**:
```
ℹ️ 事件: response.reasoning_summary_text.delta | 数据: {...}
⚠️ 推理事件但无delta | JSON: {"type":"text"}
```

**原因**: JSON结构不符合预期

**调试**:
1. 查看完整的JSON输出
2. 检查是否有不同的字段名（如 `text` 而不是 `delta`）

**可能的修复** (src/services/openai.rs):
```rust
// 尝试多个字段
if let Some(delta) = json["delta"].as_str()
    .or_else(|| json["text"].as_str())
    .or_else(|| json["content"].as_str()) {
    // 处理delta
}
```

### 问题 3: Markdown没有渲染

**症状**: UI显示纯文本，没有格式

**可能原因**:

1. **API没有返回Markdown格式**
   - 检查控制台: `📦 输出delta:` 中是否有 `**`, `#` 等符号
   - 解决: 修改prompt明确要求Markdown

2. **TextView没有正确应用样式**
   - 检查是否调用了 `.style(create_markdown_style())`
   - 代码位置: `src/ui/message_view.rs` line 194

3. **gpui-component版本问题**
   - 确认使用的是最新版本
   - 运行: `cargo update gpui-component`

---

## 验证 Markdown 支持

**快速测试用例**:

发送这条消息并观察渲染效果：

```markdown
# Markdown 测试

这是一段包含 **加粗**、*斜体* 和 ~~删除线~~ 的文本。

## 二级标题

### 三级标题

- 列表项 1
- 列表项 2
  - 嵌套项

代码块示例：
\`\`\`rust
fn main() {
    println!("Hello!");
}
\`\`\`

| 表格 | 列 |
|-----|-----|
| 1   | 2   |
```

**预期渲染**:
- ✅ 一级标题 28px 粗体
- ✅ 二级标题 24px 半粗
- ✅ 三级标题 20px 半粗
- ✅ 加粗文字显示粗体
- ✅ 斜体文字倾斜
- ✅ 代码块有背景色
- ✅ 列表有项目符号
- ✅ 表格有边框

---

## 控制台日志说明

### 正常的推理模型输出:
```
🔗 使用 Responses API 调用: https://api.ncpsnetworks.com/v1/responses
📝 模型: gpt-5-mini
🧠 推理模式: summary=auto

🧠 推理摘要delta: "I need to think about..."
🔄 实时更新推理摘要: 25 字符
🧠 更新推理摘要预览: I need to think about...

📦 输出delta: "Based on my analysis, "
📝 更新内容预览: Based on my analysis...

✅ 响应完成 | 完整响应长度: 342 | 推理长度: 156
```

### 普通模型输出（无推理）:
```
🔗 调用 Chat Completions API: https://api.ncpsnetworks.com/v1/chat/completions
📝 模型: gpt-4

📦 输出delta: "Let me explain..."
📝 更新内容预览: Let me explain...

✅ Chat Completions API 完成 - 输出: 280 字符
```

---

## 下一步

如果测试发现问题：

1. **截图控制台输出** - 包含完整的事件和数据
2. **截图UI显示** - 显示渲染效果
3. **提供测试用的prompt** - 让我可以重现问题

如果Markdown没有显示：

1. 确认控制台输出中是否包含Markdown标记
2. 如果没有，API可能不返回Markdown格式
3. 可以尝试在系统提示中添加：
   ```
   "请始终使用Markdown格式回复，包括加粗、斜体、标题等"
   ```

---

**更新**: 2025-01-05
**状态**: 已修复实时渲染，等待测试验证
