use anyhow::Result;
use gpui::{
    App, Application, AssetSource, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable,
    SharedString, Subscription, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px,
    rgb, size,
};
use gpui_component::{
    input::{InputEvent, InputState},
    *,
};
use uuid::Uuid;

mod config;
mod mcp;
mod models;
mod services;
mod tools;

use models::{Conversation, Message};
use services::{GenerationControl, GenerationPreset, OpenAIService, StorageService};

actions!(chat, [Send, NewConversation]);

struct Assets {}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        std::fs::read(path)
            .map(Into::into)
            .map_err(Into::into)
            .map(Some)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(std::fs::read_dir(path)?
            .filter_map(|entry| {
                Some(SharedString::from(
                    entry.ok()?.path().to_string_lossy().into_owned(),
                ))
            })
            .collect::<Vec<_>>())
    }
}

#[derive(Clone)]
struct PendingResponse {
    result_arc: std::sync::Arc<std::sync::Mutex<Option<(anyhow::Result<String>, StorageService)>>>,
    message_index: usize,
    conversation_id: uuid::Uuid,
    // 用于错误日志
    user_message: String,
    api_endpoint: String,
    model: String,
    // 推理状态通信
    reasoning_started: std::sync::Arc<std::sync::atomic::AtomicBool>,
    reasoning_start_instant: std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>,
    // 推理摘要内容（流式更新）
    reasoning_summary_arc: std::sync::Arc<std::sync::Mutex<String>>,
    // 流式内容累积（实时更新）
    streaming_content_arc: std::sync::Arc<std::sync::Mutex<String>>,
}

#[derive(Clone, Debug)]
struct ErrorLog {
    timestamp: String,
    error_type: String,
    error_message: String,
    user_message: String,
    api_endpoint: String,
    model: String,
}

impl ErrorLog {
    fn new(
        error: &anyhow::Error,
        user_message: String,
        api_endpoint: String,
        model: String,
    ) -> Self {
        use std::time::SystemTime;

        let timestamp = {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let secs = now.as_secs();
            let hours = (secs / 3600) % 24;
            let minutes = (secs / 60) % 60;
            let seconds = secs % 60;
            format!("{:02}:{:02}:{:02}", hours + 8, minutes, seconds) // UTC+8
        };

        Self {
            timestamp,
            error_type: "API请求失败".to_string(),
            error_message: format!("{:#}", error),
            user_message,
            api_endpoint,
            model,
        }
    }
}

// Chat Application
struct ChatApp {
    current_conversation: Conversation,
    conversations: Vec<Conversation>,
    storage: StorageService,
    openai: OpenAIService,
    // 使用 gpui-component 的专业 Input 组件
    input_state: Entity<InputState>,
    _input_subscription: Subscription,
    sidebar_search: Entity<InputState>,
    _sidebar_search_subscription: Subscription,
    focus_handle: FocusHandle,
    generation_control: GenerationControl,
    use_real_api: bool,
    // 开关：是否使用 Responses API（仅由用户控制）
    use_responses_api: bool,
    is_loading: bool,
    streaming_content: String,
    pending_response: Option<PendingResponse>,
    pending_clear_input: bool,
    // 帧泵：在流式期间按帧触发刷新，保证实时渲染
    streaming_pump_running: bool,
    // 调试信息
    debug_info: String,
    last_error: Option<String>,
    error_log: Option<ErrorLog>,
    show_error_dialog: bool,
    // 推理阶段相关
    is_reasoning: bool,
    reasoning_summary: Option<String>,
    reasoning_start_time: Option<std::time::Instant>,
    reasoning_duration: Option<f64>,
    show_reasoning_expanded: bool,
    // 追踪哪些消息的推理摘要是展开的（使用消息timestamp）
    expanded_reasoning_messages: std::collections::HashSet<i64>,
    // 性能优化：上次UI更新时间（用于节流）
    last_ui_update: std::time::Instant,
    inspector_visible: bool,
    conversation_filter: String,
}

impl EventEmitter<()> for ChatApp {}

impl Focusable for ChatApp {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ChatApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let storage = StorageService::default();
        let mut conversations = storage.load_conversations().unwrap_or_default();

        // Add demo conversation if no conversations exist
        if conversations.is_empty() {
            let demo = Self::create_demo_conversation();
            let _ = storage.save_conversation(&demo);
            conversations.push(demo.clone());
        }

        let current_conversation = conversations.first().cloned().unwrap_or_default();

        // Check if OPENAI_API_KEY is set
        let api_key_set = std::env::var("OPENAI_API_KEY").is_ok();
        let api_base = std::env::var("OPENAI_API_BASE").ok();
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4".to_string());

        let use_real_api = api_key_set;

        // 构建调试信息
        let debug_info = format!(
            "🔧 调试信息:\n\
             API Key: {}\n\
             API Base: {}\n\
             Model: {}\n\
             模式: {}",
            if api_key_set {
                "✅ 已配置"
            } else {
                "❌ 未配置"
            },
            api_base
                .as_ref()
                .unwrap_or(&"默认 (OpenAI 官方)".to_string()),
            model,
            if use_real_api {
                "真实API"
            } else {
                "模拟模式"
            }
        );

        println!("{}", debug_info);

        // 创建专业的 InputState（多行 + 自动增高）
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Ask anything… (Shift+Enter 换行)")
                .auto_grow(3, 10)
        });

        let sidebar_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search conversations"));

        // 订阅输入事件（需要 window 获取修饰键状态）
        let _input_subscription = cx.subscribe_in(
            &input_state,
            window,
            move |this, _input, event, window, cx| {
                match event {
                    InputEvent::Change => {
                        cx.notify();
                    }
                    InputEvent::PressEnter { secondary } => {
                        let modifiers = window.modifiers();
                        let shift_pressed = modifiers.shift;

                        // Shift + Enter: 仅换行，不发送
                        if shift_pressed && !secondary {
                            return;
                        }

                        this.trim_input_trailing_newlines(window, cx);
                        this.send_message(cx);
                    }
                    InputEvent::Focus | InputEvent::Blur => {
                        // 可以在这里添加焦点状态处理
                    }
                }
            },
        );

        let _sidebar_search_subscription = cx.subscribe_in(
            &sidebar_search,
            window,
            |this: &mut Self, state, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.conversation_filter = state.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );

        let mut app = Self {
            current_conversation,
            conversations,
            storage,
            openai: OpenAIService::new(),
            input_state,
            _input_subscription,
            sidebar_search,
            _sidebar_search_subscription,
            focus_handle: cx.focus_handle(),
            generation_control: GenerationControl::default(),
            use_real_api,
            use_responses_api: false,
            is_loading: false,
            streaming_content: String::new(),
            pending_response: None,
            pending_clear_input: false,
            streaming_pump_running: false,
            debug_info,
            last_error: None,
            error_log: None,
            show_error_dialog: false,
            // 推理阶段初始化
            is_reasoning: false,
            reasoning_summary: None,
            reasoning_start_time: None,
            reasoning_duration: None,
            show_reasoning_expanded: false,
            expanded_reasoning_messages: std::collections::HashSet::new(),
            last_ui_update: std::time::Instant::now(),
            inspector_visible: false,
            conversation_filter: String::new(),
        };

        // 读取服务初始配置（来自环境变量）
        app.use_responses_api = app.openai.use_responses_api();
        app
    }

    fn create_demo_conversation() -> Conversation {
        // 创建空的对话，让用户从干净的状态开始
        Conversation::new()
    }

    fn sync_current_conversation(&mut self, bring_to_front: bool) {
        if let Some(pos) = self
            .conversations
            .iter()
            .position(|c| c.id == self.current_conversation.id)
        {
            self.conversations[pos] = self.current_conversation.clone();
            if bring_to_front && pos != 0 {
                let conv = self.conversations.remove(pos);
                self.conversations.insert(0, conv);
            }
        } else if bring_to_front {
            self.conversations
                .insert(0, self.current_conversation.clone());
        } else {
            self.conversations.push(self.current_conversation.clone());
        }
    }

    fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Save current conversation if it has messages
        if !self.current_conversation.is_empty() {
            let _ = self.storage.save_conversation(&self.current_conversation);

            // Update or add to conversations list
            if let Some(pos) = self
                .conversations
                .iter()
                .position(|c| c.id == self.current_conversation.id)
            {
                self.conversations[pos] = self.current_conversation.clone();
            } else {
                self.conversations
                    .insert(0, self.current_conversation.clone());
            }
        }

        // Create new conversation
        self.current_conversation = Conversation::new();
        self.sync_current_conversation(true);

        // 清空输入框
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });

        cx.notify();
    }

    fn load_conversation(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        // Save current conversation first
        if !self.current_conversation.is_empty() {
            let _ = self.storage.save_conversation(&self.current_conversation);

            // Update in list
            if let Some(pos) = self
                .conversations
                .iter()
                .position(|c| c.id == self.current_conversation.id)
            {
                self.conversations[pos] = self.current_conversation.clone();
            }
        }

        // Load selected conversation
        if let Some(conv) = self.conversations.iter().find(|c| c.id == id) {
            self.current_conversation = conv.clone();
            self.sync_current_conversation(true);

            // 清空输入框
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });

            cx.notify();
        }
    }

    fn trim_input_trailing_newlines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let current = state.value().to_string();
            let trimmed = current.trim_end_matches('\n').to_string();
            if trimmed.len() != current.len() {
                state.set_value(trimmed, window, cx);
            }
        });
    }

    fn send_message(&mut self, cx: &mut Context<Self>) {
        // 获取输入框的文本
        let input_text = self.input_state.read(cx).value().to_string();

        println!("📤 发送消息: {:?}", input_text);
        println!("🔄 当前加载状态: {}", self.is_loading);
        println!("🔑 使用真实API: {}", self.use_real_api);

        if input_text.trim().is_empty() {
            println!("⚠️  输入为空，取消发送");
            return;
        }

        if self.is_loading {
            println!("⚠️  正在加载中，取消发送");
            return;
        }

        let user_message = Message::new_user(input_text.clone());
        self.current_conversation.add_message(user_message);

        // 标记需要清空输入框（将在下次render时清空）
        self.pending_clear_input = true;

        // 🔄 修改：不再立即设置is_loading，而是等待接收到API响应
        self.is_loading = false; // 发送阶段不显示加载
        self.is_reasoning = false; // 推理阶段由API响应触发
        self.reasoning_summary = None;
        self.reasoning_start_time = None;
        self.reasoning_duration = None;
        self.streaming_content.clear();
        self.last_error = None;

        println!("✅ 消息已添加，开始API调用...");

        // Add empty assistant message that will be filled by API response
        let assistant_message = Message::new_assistant(String::new());
        self.current_conversation.add_message(assistant_message);
        self.sync_current_conversation(true);
        let _ = self.storage.save_conversation(&self.current_conversation);

        let message_index = self.current_conversation.messages.len() - 1;

        cx.notify();

        // Clone what we need for the async task
        let messages = self.current_conversation.messages.clone();
        let openai = self.openai.clone();
        let use_real_api = self.use_real_api;

        // 收集API配置信息用于错误日志
        let api_endpoint = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4".to_string());
        let user_msg_for_log = input_text.clone();

        // Create Arc for conversation ID to update later
        let conv_id = self.current_conversation.id;
        let storage = self.storage.clone();

        // For simplicity, we'll do synchronous waiting in background thread
        // and update the message directly
        let task_result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let task_result_clone = task_result.clone();

        // 创建推理状态通信用的Arc
        let reasoning_started = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reasoning_started_clone = reasoning_started.clone();
        let reasoning_start_instant = std::sync::Arc::new(std::sync::Mutex::new(None));
        let reasoning_start_instant_clone = reasoning_start_instant.clone();

        // 创建推理摘要内容的Arc（用于流式更新）
        let reasoning_summary_arc = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let reasoning_summary_arc_clone = reasoning_summary_arc.clone();

        // 创建流式内容累积的Arc（实时更新显示）
        let streaming_content_arc = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let streaming_content_arc_clone = streaming_content_arc.clone();

        // Spawn background thread for API call
        std::thread::spawn(move || {
            println!("🚀 后台线程启动，准备调用API...");
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                if use_real_api {
                    // Use OpenAI Responses API (supports reasoning summary!)
                    println!("📡 调用 OpenAI Responses API（支持 reasoning summary）...");

                    let first_chunk = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    let first_chunk_clone = first_chunk.clone();
                    let first_reasoning = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    let first_reasoning_clone = first_reasoning.clone();

                    let reasoning_started_clone2 = reasoning_started_clone.clone();
                    let reasoning_start_instant_clone2 = reasoning_start_instant_clone.clone();
                    // 供推理摘要回调使用的克隆（当先到达推理摘要时也要触发UI进入思考阶段）
                    let reasoning_started_clone3 = reasoning_started_clone.clone();
                    let reasoning_start_instant_clone3 = reasoning_start_instant_clone.clone();
                    let streaming_content_arc_clone2 = streaming_content_arc_clone.clone();
                    let reasoning_summary_arc_clone2 = reasoning_summary_arc_clone.clone();

                    // 使用 Responses API，支持 reasoning summary
                    match openai.get_streaming_completion_native(
                        &messages,
                        // 回调1: 处理普通内容
                        move |chunk| {
                            // 收到第一个内容chunk时标记推理开始
                            if first_chunk_clone.load(std::sync::atomic::Ordering::SeqCst) {
                                reasoning_started_clone2.store(true, std::sync::atomic::Ordering::SeqCst);
                                *reasoning_start_instant_clone2.lock().unwrap() = Some(std::time::Instant::now());
                                println!("🚀 响应开始！（收到第一个内容chunk）");
                                first_chunk_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                            }

                            // 实时累积流式内容
                            let mut content = streaming_content_arc_clone2.lock().unwrap();
                            content.push_str(&chunk);
                            println!("📦 内容chunk: {} 字符，累积: {}", chunk.len(), content.len());
                        },
                        // 回调2: 处理推理摘要（reasoning_summary_text）
                        move |reasoning_chunk| {
                            // 若先收到推理摘要，也标记思考阶段开始
                            if first_reasoning_clone.load(std::sync::atomic::Ordering::SeqCst) {
                                reasoning_started_clone3.store(true, std::sync::atomic::Ordering::SeqCst);
                                let mut guard = reasoning_start_instant_clone3.lock().unwrap();
                                if guard.is_none() { *guard = Some(std::time::Instant::now()); }
                            }
                            if first_reasoning_clone.load(std::sync::atomic::Ordering::SeqCst) {
                                println!("🧠 检测到推理摘要！开始流式显示推理过程");
                                first_reasoning_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                            }

                            // 实时累积推理摘要
                            let mut summary = reasoning_summary_arc_clone2.lock().unwrap();
                            summary.push_str(&reasoning_chunk);
                            println!("🧠 推理摘要chunk: {} 字符，累积: {}", reasoning_chunk.len(), summary.len());
                        }
                    ).await {
                        Ok(result) => {
                            println!("✅ Responses API 调用成功！");
                            println!("   - 输出长度: {} 字符", result.content.len());

                            // 推理摘要已经在回调中实时更新到 reasoning_summary_arc_clone
                            // 这里只需要打印确认信息
                            if let Some(ref summary) = result.reasoning_summary {
                                println!("   - 推理摘要: {} 字符", summary.len());
                                println!("   ✅ 推理模型 - 已提取推理过程！");
                            } else {
                                println!("   - 无推理摘要（普通模型）");
                            }

                            // 检测工具调用
                            if result.has_tool_calls() {
                                println!("   🔧 检测到 {} 个工具调用（暂不执行）", result.tool_calls.len());
                                for tool_call in &result.tool_calls {
                                    println!("      - {}", tool_call.name);
                                }
                            }

                            Ok(result.content)
                        }
                        Err(e) => {
                            println!("❌ API调用失败: {}", e);
                            Err(e)
                        }
                    }
                } else {
                    // Mock response - 不预设推理摘要，模拟真实API行为
                    println!("🎭 使用模拟模式响应...");

                    // 标记响应开始（开始生成内容）
                    reasoning_started_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    *reasoning_start_instant_clone.lock().unwrap() = Some(std::time::Instant::now());
                    println!("🚀 模拟响应生成开始！");

                    // ⚠️ 注意：模拟模式不生成推理摘要
                    // 只有真实的推理模型（如 o1）才会返回 reasoning summary
                    // 这样做是为了：
                    // 1. 模拟真实API行为（普通模型没有推理摘要）
                    // 2. 让UI按实际数据决定是否显示"思考过程"
                    // 3. 为未来接入MCP Server做准备

                    // 模拟流式输出回复内容
                    let response = "**Mock AI Response**\n\nThis is a simulated response. \n\n\
                    ✨ The chat application features:\n\
                    - ✅ 专业输入框（支持中文IME）\n\
                    - ✅ ChatGPT风格UI\n\
                    - ✅ Markdown rendering\n\
                    - ✅ Conversation management\n\
                    - ✅ Auto-save functionality\n\
                    - ✅ Scrollable message area\n\n\
                    💡 提示：设置 OPENAI_API_KEY 环境变量来启用真实API调用。\n\
                    💡 注意：模拟模式不生成推理摘要，只有真实的推理模型（如OpenAI o1）才会显示思考过程。";

                    // 模拟逐字输出
                    let chars: Vec<char> = response.chars().collect();
                    for i in (0..chars.len()).step_by(5) {  // 每次输出5个字符
                        let end = std::cmp::min(i + 5, chars.len());
                        let chunk: String = chars[i..end].iter().collect();
                        {
                            let mut content = streaming_content_arc_clone.lock().unwrap();
                            content.push_str(&chunk);
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(30)).await;  // 模拟延迟
                    }

                    println!("✅ 模拟响应已生成");
                    Ok(response.to_string())
                }
            });

            // Store result
            println!("💾 存储API结果...");
            *task_result_clone.lock().unwrap() = Some((result, storage));
            println!("✅ 后台线程完成");
        });

        // Store the Arc in the app state for polling during render
        self.pending_response = Some(PendingResponse {
            result_arc: task_result,
            message_index,
            conversation_id: conv_id,
            user_message: user_msg_for_log,
            api_endpoint,
            model,
            reasoning_started,
            reasoning_start_instant,
            reasoning_summary_arc,
            streaming_content_arc,
        });
    }

    fn preset_label(preset: &GenerationPreset) -> &'static str {
        match preset {
            GenerationPreset::Precise => "Precise",
            GenerationPreset::Balanced => "Balanced",
            GenerationPreset::Creative => "Creative",
            GenerationPreset::Concise => "Concise",
            GenerationPreset::Detailed => "Detailed",
            GenerationPreset::Custom(_) => "Custom",
        }
    }

    fn set_generation_preset(&mut self, preset: GenerationPreset, cx: &mut Context<Self>) {
        self.generation_control.preset = preset;
        cx.notify();
    }

    fn cycle_generation_preset(&mut self, cx: &mut Context<Self>) {
        let next = match self.generation_control.preset.clone() {
            GenerationPreset::Precise => GenerationPreset::Balanced,
            GenerationPreset::Balanced => GenerationPreset::Creative,
            GenerationPreset::Creative => GenerationPreset::Concise,
            GenerationPreset::Concise => GenerationPreset::Detailed,
            GenerationPreset::Detailed => GenerationPreset::Precise,
            GenerationPreset::Custom(_) => GenerationPreset::Balanced,
        };

        self.generation_control.preset = next;
        cx.notify();
    }

    fn check_pending_response(&mut self, cx: &mut Context<Self>) {
        if let Some(pending) = self.pending_response.clone() {
            // 🧠 检查推理是否已开始
            if pending
                .reasoning_started
                .load(std::sync::atomic::Ordering::SeqCst)
                && !self.is_reasoning
            {
                self.is_reasoning = true;
                if let Some(start_instant) = *pending.reasoning_start_instant.lock().unwrap() {
                    self.reasoning_start_time = Some(start_instant);
                    println!("🎯 UI检测到推理开始！");
                }
                cx.notify();
            }

            // 📝 实时更新推理摘要和流式内容（使用节流优化性能）
            if self.is_reasoning {
                // ⚡ 性能优化：限制UI更新频率到每100ms一次
                let now = std::time::Instant::now();
                let time_since_last_update = now.duration_since(self.last_ui_update);
                let should_throttle = time_since_last_update.as_millis() < 100;

                if !should_throttle {
                    let current_reasoning_summary =
                        pending.reasoning_summary_arc.lock().unwrap().clone();
                    let current_streaming_content =
                        pending.streaming_content_arc.lock().unwrap().clone();

                    let mut needs_notify = false;

                    // 更新推理摘要
                    if !current_reasoning_summary.is_empty() {
                        if self
                            .reasoning_summary
                            .as_ref()
                            .map_or(true, |s| s != &current_reasoning_summary)
                        {
                            self.reasoning_summary = Some(current_reasoning_summary.clone());
                            needs_notify = true;
                        }
                    }

                    // 更新流式内容和消息中的推理摘要
                    if let Some(msg) = self
                        .current_conversation
                        .messages
                        .get_mut(pending.message_index)
                    {
                        // 更新内容
                        if msg.content != current_streaming_content {
                            msg.content = current_streaming_content.clone();
                            needs_notify = true;
                        }

                        // 🧠 实时更新消息中的推理摘要
                        if !current_reasoning_summary.is_empty() {
                            let should_update = match &msg.reasoning_summary {
                                Some(existing) => existing != &current_reasoning_summary,
                                None => true,
                            };

                            if should_update {
                                msg.reasoning_summary = Some(current_reasoning_summary);
                                needs_notify = true;
                            }
                        }
                    }

                    if needs_notify {
                        self.last_ui_update = now;
                        cx.notify(); // 触发UI重新渲染
                    }
                }
            }

            // 🔄 检查响应是否完成
            let mut result_lock = pending.result_arc.lock().unwrap();

            if let Some((result, storage)) = result_lock.take() {
                // 🧠 推理完成，获取推理摘要和计算耗时
                let final_summary = pending.reasoning_summary_arc.lock().unwrap().clone();
                if !final_summary.is_empty() {
                    self.reasoning_summary = Some(final_summary);
                    println!("📝 获取推理摘要");
                }

                if let Some(start_time) = self.reasoning_start_time {
                    let duration = start_time.elapsed().as_secs_f64();
                    self.reasoning_duration = Some(duration);
                    println!("⏱️  推理完成，耗时: {:.2}秒", duration);
                }
                self.is_reasoning = false;
                println!("📥 收到API响应");
                self.is_loading = false;

                if self.current_conversation.id == pending.conversation_id {
                    match result {
                        Ok(content) => {
                            println!("✅ 成功接收响应，长度: {} 字符", content.len());
                            if let Some(msg) = self
                                .current_conversation
                                .messages
                                .get_mut(pending.message_index)
                            {
                                msg.content = content;
                                // ⚠️ 不要再次更新推理摘要！它已经在实时更新中设置了
                                // msg.reasoning_summary 已经在流式传输时更新
                                msg.reasoning_duration = self.reasoning_duration;
                                if msg
                                    .reasoning_summary
                                    .as_ref()
                                    .map_or(true, |s| s.trim().is_empty())
                                {
                                    let summary =
                                        pending.reasoning_summary_arc.lock().unwrap().clone();
                                    if !summary.is_empty() {
                                        msg.reasoning_summary = Some(summary);
                                    }
                                }

                                if let Some(summary) = &msg.reasoning_summary {
                                    println!("💾 最终推理摘要长度: {} 字符", summary.len());
                                }
                            }
                            self.last_error = None;
                            // 清除错误日志和对话框
                            self.error_log = None;
                            self.show_error_dialog = false;
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            println!("❌ API错误: {}", error_msg);

                            // 创建详细的错误日志
                            let error_log = ErrorLog::new(
                                &e,
                                pending.user_message.clone(),
                                pending.api_endpoint.clone(),
                                pending.model.clone(),
                            );

                            println!("📋 错误日志已创建: {:?}", error_log);

                            if let Some(msg) = self
                                .current_conversation
                                .messages
                                .get_mut(pending.message_index)
                            {
                                msg.content = format!(
                                    "**❌ 错误**\n\n{}\n\n---\n💡 查看上方红色错误框了解详情",
                                    error_msg
                                );
                                if msg
                                    .reasoning_summary
                                    .as_ref()
                                    .map_or(true, |s| s.trim().is_empty())
                                {
                                    let summary =
                                        pending.reasoning_summary_arc.lock().unwrap().clone();
                                    if !summary.is_empty() {
                                        msg.reasoning_summary = Some(summary);
                                    }
                                }
                            }

                            self.last_error = Some(error_msg);
                            self.error_log = Some(error_log);
                        }
                    }

                    self.streaming_content.clear();
                    self.sync_current_conversation(true);
                    let _ = storage.save_conversation(&self.current_conversation);
                }

                self.pending_response = None;
                // 结束帧泵
                self.streaming_pump_running = false;
                cx.notify();
            } else {
                cx.notify();
            }
        }
    }

    // 在流式期间按帧触发刷新，直到 pending_response 结束
    fn ensure_streaming_pump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_response.is_some() && !self.streaming_pump_running {
            self.streaming_pump_running = true;
            self.schedule_next_frame(window, cx);
        }
    }

    fn schedule_next_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.on_next_frame(window, |this, window, cx| {
            if this.pending_response.is_some() {
                // 触发一次渲染，让 UI 消化最新的流式内容
                cx.notify();
                // 持续调度下一帧
                this.schedule_next_frame(window, cx);
            } else {
                this.streaming_pump_running = false;
                cx.notify();
            }
        });
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.check_pending_response(cx);
        self.ensure_streaming_pump(window, cx);

        if self.pending_clear_input {
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            self.pending_clear_input = false;
        }

        let placeholder_text = if self.current_conversation.messages.is_empty() {
            "UI 已移除，准备重新设计。"
        } else {
            "当前对话仍在运行，但界面已清空，等待新的设计。"
        };

        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("UI removed"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x666666))
                    .child(placeholder_text),
            )
    }
}

fn main() {
    Application::new()
        .with_assets(Assets {})
        .run(|cx: &mut App| {
            gpui_component::init(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1400.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(1100.), px(720.))),
                ..Default::default()
            };

            cx.open_window(options, |window, cx| {
                cx.activate(false);
                // 先创建 ChatApp
                let chat_app = cx.new(|cx| ChatApp::new(window, cx));
                // 将 ChatApp 包装在 Root 中（gpui-component 要求 Root 作为顶层view）
                cx.new(|cx| Root::new(chat_app.into(), window, cx))
            })
            .unwrap();
        });
}
