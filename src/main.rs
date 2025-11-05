use anyhow::Result;
use gpui::{
    actions, App, Application, AssetSource, Bounds, Context, Entity, EventEmitter, Focusable,
    FocusHandle, SharedString, Subscription, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use gpui_component::{*, input::{Input, InputEvent, InputState}};
use uuid::Uuid;

mod models;
mod services;
mod ui;

use models::{Conversation, Message};
use services::{OpenAIService, StorageService};
use ui::{render_message_list, render_sidebar};

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
    fn new(error: &anyhow::Error, user_message: String, api_endpoint: String, model: String) -> Self {
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
    focus_handle: FocusHandle,
    use_real_api: bool,
    is_loading: bool,
    streaming_content: String,
    pending_response: Option<PendingResponse>,
    pending_clear_input: bool,
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
            if api_key_set { "✅ 已配置" } else { "❌ 未配置" },
            api_base.as_ref().unwrap_or(&"默认 (OpenAI 官方)".to_string()),
            model,
            if use_real_api { "真实API" } else { "模拟模式" }
        );

        println!("{}", debug_info);

        // 创建专业的 InputState（多行模式）
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("输入消息... (Shift+Enter 换行, Enter 发送)")
                .multi_line()
        });

        // 订阅输入事件
        let _input_subscription = cx.subscribe(&input_state, move |this, _input, event, cx| {
            match event {
                InputEvent::Change => {
                    cx.notify();
                }
                InputEvent::PressEnter { secondary } => {
                    if !secondary {
                        this.send_message(cx);
                    }
                }
                InputEvent::Focus | InputEvent::Blur => {
                    // 可以在这里添加焦点状态处理
                }
            }
        });

        Self {
            current_conversation,
            conversations,
            storage,
            openai: OpenAIService::new(),
            input_state,
            _input_subscription,
            focus_handle: cx.focus_handle(),
            use_real_api,
            is_loading: false,
            streaming_content: String::new(),
            pending_response: None,
            pending_clear_input: false,
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
        }
    }

    fn create_demo_conversation() -> Conversation {
        // 创建空的对话，让用户从干净的状态开始
        Conversation::new()
    }

    fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Save current conversation if it has messages
        if !self.current_conversation.is_empty() {
            let _ = self.storage.save_conversation(&self.current_conversation);

            // Update or add to conversations list
            if let Some(pos) = self.conversations.iter().position(|c| c.id == self.current_conversation.id) {
                self.conversations[pos] = self.current_conversation.clone();
            } else {
                self.conversations.insert(0, self.current_conversation.clone());
            }
        }

        // Create new conversation
        self.current_conversation = Conversation::new();

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
            if let Some(pos) = self.conversations.iter().position(|c| c.id == self.current_conversation.id) {
                self.conversations[pos] = self.current_conversation.clone();
            }
        }

        // Load selected conversation
        if let Some(conv) = self.conversations.iter().find(|c| c.id == id) {
            self.current_conversation = conv.clone();

            // 清空输入框
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });

            cx.notify();
        }
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
        self.is_loading = false;  // 发送阶段不显示加载
        self.is_reasoning = false;  // 推理阶段由API响应触发
        self.reasoning_summary = None;
        self.reasoning_start_time = None;
        self.reasoning_duration = None;
        self.streaming_content.clear();
        self.last_error = None;

        println!("✅ 消息已添加，开始API调用...");

        // Add empty assistant message that will be filled by API response
        let assistant_message = Message::new_assistant(String::new());
        self.current_conversation.add_message(assistant_message);

        let message_index = self.current_conversation.messages.len() - 1;

        cx.notify();

        // Clone what we need for the async task
        let messages = self.current_conversation.messages.clone();
        let openai = self.openai.clone();
        let use_real_api = self.use_real_api;

        // 收集API配置信息用于错误日志
        let api_endpoint = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4".to_string());
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
                    // Use real OpenAI API with streaming
                    println!("📡 调用真实 OpenAI API（流式模式）...");

                    let first_chunk = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
                    let first_chunk_clone = first_chunk.clone();
                    let reasoning_started_clone2 = reasoning_started_clone.clone();
                    let reasoning_start_instant_clone2 = reasoning_start_instant_clone.clone();
                    let streaming_content_arc_clone2 = streaming_content_arc_clone.clone();

                    match openai.get_streaming_completion(&messages, move |chunk| {
                        // 🧠 收到第一个chunk时才标记推理开始
                        if first_chunk_clone.load(std::sync::atomic::Ordering::SeqCst) {
                            reasoning_started_clone2.store(true, std::sync::atomic::Ordering::SeqCst);
                            *reasoning_start_instant_clone2.lock().unwrap() = Some(std::time::Instant::now());
                            println!("🧠 推理阶段开始！（收到第一个响应chunk）");
                            first_chunk_clone.store(false, std::sync::atomic::Ordering::SeqCst);
                        }

                        // 📝 实时累积流式内容
                        let mut content = streaming_content_arc_clone2.lock().unwrap();
                        content.push_str(&chunk);
                        println!("📦 收到chunk: {} 字符，累积长度: {}", chunk.len(), content.len());
                    }).await {
                        Ok((content, reasoning_summary_opt)) => {
                            println!("✅ 流式API调用成功，总响应长度: {} 字符", content.len());

                            // ⚠️ 只使用API实际返回的推理摘要，不生成假数据
                            // 如果API返回了reasoning summary（如o1模型），则保存并显示
                            // 如果没有返回（如gpt-4等普通模型），则不显示"思考过程"
                            if let Some(summary) = reasoning_summary_opt {
                                *reasoning_summary_arc_clone.lock().unwrap() = summary;
                                println!("📝 API返回了推理摘要，已保存");
                            } else {
                                println!("ℹ️  API未返回推理摘要（这是正常的，普通模型不提供推理过程）");
                            }

                            Ok(content)
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

    fn check_pending_response(&mut self, cx: &mut Context<Self>) {
        if let Some(pending) = self.pending_response.clone() {
            // 🧠 检查推理是否已开始
            if pending.reasoning_started.load(std::sync::atomic::Ordering::SeqCst) && !self.is_reasoning {
                self.is_reasoning = true;
                if let Some(start_instant) = *pending.reasoning_start_instant.lock().unwrap() {
                    self.reasoning_start_time = Some(start_instant);
                    println!("🎯 UI检测到推理开始！");
                }
                cx.notify();
            }

            // 📝 实时更新推理摘要（在思考阶段实时显示）
            if self.is_reasoning {
                let current_reasoning_summary = pending.reasoning_summary_arc.lock().unwrap().clone();
                if !current_reasoning_summary.is_empty() {
                    if self.reasoning_summary.as_ref().map_or(true, |s| s != &current_reasoning_summary) {
                        self.reasoning_summary = Some(current_reasoning_summary);
                        cx.notify();
                    }
                }

                // 📝 实时更新流式内容（在推理阶段进行）
                let current_streaming_content = pending.streaming_content_arc.lock().unwrap().clone();
                if let Some(msg) = self.current_conversation.messages.get_mut(pending.message_index) {
                    if msg.content != current_streaming_content {
                        msg.content = current_streaming_content;
                        cx.notify();  // 触发UI重新渲染
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
                            if let Some(msg) = self.current_conversation.messages.get_mut(pending.message_index) {
                                msg.content = content;
                                // 保存推理摘要和耗时到消息中
                                msg.reasoning_summary = self.reasoning_summary.clone();
                                msg.reasoning_duration = self.reasoning_duration;

                                if let Some(summary) = &msg.reasoning_summary {
                                    println!("💾 保存推理摘要到消息，长度: {} 字符", summary.len());
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

                            if let Some(msg) = self.current_conversation.messages.get_mut(pending.message_index) {
                                msg.content = format!("**❌ 错误**\n\n{}\n\n---\n💡 查看上方红色错误框了解详情", error_msg);
                            }

                            self.last_error = Some(error_msg);
                            self.error_log = Some(error_log);
                            self.show_error_dialog = true;
                        }
                    }

                    self.streaming_content.clear();
                    let _ = storage.save_conversation(&self.current_conversation);
                }

                self.pending_response = None;
                cx.notify();
            } else {
                cx.notify();
            }
        }
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 处理pending API响应
        self.check_pending_response(cx);

        // 处理待清空的输入框
        if self.pending_clear_input {
            self.input_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
            self.pending_clear_input = false;
        }

        let messages = self.current_conversation.messages.clone();
        let current_id = self.current_conversation.id;
        let has_messages = !messages.is_empty();

        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(rgb(0xffffff))
            .child(
                // Sidebar
                render_sidebar(
                    &self.conversations,
                    current_id,
                    |this: &mut Self, _, window, cx| {
                        this.new_conversation(window, cx);
                    },
                    |this: &mut Self, id, _, window, cx| {
                        this.load_conversation(id, window, cx);
                    },
                    cx,
                )
            )
            .child(
                // Main Chat Area - 使用flex列布局
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        // Chat Header - 固定高度
                        div()
                            .w_full()
                            .h(px(64.))
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .px_6()
                            .border_b_1()
                            .border_color(rgb(0xdee2e6))
                            .bg(rgb(0xffffff))
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x212529))
                                    .child(self.current_conversation.title.clone())
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x6c757d))
                                    .child("Markdown enabled • Prototype")
                            )
                    )
                    .child(
                        // 调试信息面板
                        div()
                            .w_full()
                            .px_6()
                            .py_2()
                            .bg(rgb(0xfff3cd))  // 黄色背景
                            .border_b_1()
                            .border_color(rgb(0xffc107))
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_4()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(0x664d03))
                                            .child("🔧 调试信息:")
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x664d03))
                                            .child(if self.use_real_api { "API模式: ✅ 真实API" } else { "API模式: 🎭 模拟" })
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(0x664d03))
                                            .child(if self.is_loading { "状态: ⏳ 加载中..." } else { "状态: ✅ 就绪" })
                                    )
                                    .when_some(self.last_error.clone(), |d, err| {
                                        d.child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .text_color(rgb(0xdc3545))
                                                .child(format!("错误: {}", err))
                                        )
                                    })
                            )
                    )
                    .when(self.show_error_dialog, |d| {
                        // 🚨 浅红色错误提示框
                        d.child(
                            div()
                                .w_full()
                                .px_6()
                                .py_4()
                                .bg(rgb(0xf8d7da))  // 浅红色背景
                                .border_b_1()
                                .border_color(rgb(0xf5c2c7))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .items_start()
                                        .child(
                                            div()
                                                .flex_1()
                                                .flex()
                                                .flex_col()
                                                .gap_3()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_row()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_base()
                                                                .font_weight(gpui::FontWeight::BOLD)
                                                                .text_color(rgb(0x842029))
                                                                .child("🚨 API 请求失败")
                                                        )
                                                        .when_some(self.error_log.as_ref(), |d, log| {
                                                            d.child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x6c757d))
                                                                    .child(format!("时间: {}", log.timestamp))
                                                            )
                                                        })
                                                )
                                                .when_some(self.error_log.as_ref(), |d, log| {
                                                    d.child(
                                                        div()
                                                            .flex()
                                                            .flex_col()
                                                            .gap_2()
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x58151c))
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_col()
                                                                            .gap_1()
                                                                            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("📤 您的消息:"))
                                                                            .child(div().child(format!("\"{}\"", log.user_message)))
                                                                    )
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x58151c))
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_col()
                                                                            .gap_1()
                                                                            .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("❌ 错误详情:"))
                                                                            .child(div().child(log.error_message.clone()))
                                                                    )
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_xs()
                                                                    .text_color(rgb(0x6c757d))
                                                                    .child(
                                                                        div()
                                                                            .flex()
                                                                            .flex_row()
                                                                            .gap_4()
                                                                            .child(div().child(format!("🌐 API: {}", log.api_endpoint)))
                                                                            .child(div().child(format!("🤖 模型: {}", log.model)))
                                                                    )
                                                            )
                                                    )
                                                })
                                        )
                                        .child(
                                            // 关闭按钮
                                            div()
                                                .px_2()
                                                .py_1()
                                                .cursor_pointer()
                                                .rounded(px(4.))
                                                .hover(|style| style.bg(rgb(0xf5c2c7)))
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                    this.show_error_dialog = false;
                                                    cx.notify();
                                                }))
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .font_weight(gpui::FontWeight::BOLD)
                                                        .text_color(rgb(0x842029))
                                                        .child("✕")
                                                )
                                        )
                                )
                        )
                    })
                    .child(
                        // Messages Area - 使用flex-1占用剩余空间，添加垂直滚动
                        div()
                            .id("messages-scroll-area")
                            .flex_1()
                            .w_full()
                            .overflow_y_scroll()  // 修改：使用overflow_y_scroll而不是overflow_hidden
                            .p_6()
                            .when(!has_messages, |d| {
                                d.flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap_4()
                                            .child(
                                                div()
                                                    .text_2xl()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0x495057))
                                                    .child("🧪 滚动测试模式")
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(rgb(0x6c757d))
                                                    .child("已加载测试消息 - 请尝试滚动")
                                            )
                                    )
                            })
                            .when(has_messages, |d| {
                                d.child(render_message_list(
                                    &messages,
                                    &self.expanded_reasoning_messages,
                                    window,
                                    cx,
                                    |this: &mut Self, timestamp, _, cx| {
                                        // Toggle展开/收起状态
                                        if this.expanded_reasoning_messages.contains(&timestamp) {
                                            this.expanded_reasoning_messages.remove(&timestamp);
                                        } else {
                                            this.expanded_reasoning_messages.insert(timestamp);
                                        }
                                        cx.notify();
                                    },
                                ))
                            })
                            .when(self.is_reasoning && self.reasoning_summary.is_some(), |d| {
                                // 🧠 只有当有推理摘要数据时才显示"思考过程"框
                                // 这符合 Raycast 的行为：有 reasoning summary 就显示，没有就跳过
                                let elapsed = if let Some(start_time) = self.reasoning_start_time {
                                    start_time.elapsed().as_secs_f64()
                                } else {
                                    0.0
                                };

                                // 获取当前累积的推理摘要
                                let current_summary = self.reasoning_summary.clone().unwrap_or_default();

                                d.child(
                                    div()
                                        .mt_4()
                                        .w_full()
                                        .child(
                                            div()
                                                .max_w(px(700.))
                                                .p_4()
                                                .rounded_lg()
                                                .bg(rgb(0xf5f5f5))  // 浅灰色背景
                                                .border_1()
                                                .border_color(rgb(0xd0d0d0))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .flex_col()
                                                        .gap_3()
                                                        .child(
                                                            // 顶部：标题和计时器
                                                            div()
                                                                .flex()
                                                                .flex_row()
                                                                .items_center()
                                                                .justify_between()
                                                                .child(
                                                                    div()
                                                                        .flex()
                                                                        .items_center()
                                                                        .gap_2()
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(rgb(0x2d2d2d))
                                                                                .font_weight(gpui::FontWeight::BOLD)
                                                                                .child("▼")
                                                                        )
                                                                        .child(
                                                                            div()
                                                                                .text_xs()
                                                                                .text_color(rgb(0x2d2d2d))
                                                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                                                .child("🧠 推理过程")
                                                                        )
                                                                )
                                                                .child(
                                                                    // ⏱️ 实时计时器
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(rgb(0x555555))
                                                                        .font_weight(gpui::FontWeight::MEDIUM)
                                                                        .child(format!("⏱️ {:.1}s", elapsed))
                                                                )
                                                        )
                                                        .when(!current_summary.is_empty(), |d| {
                                                            // 显示累积的推理摘要内容（流式）
                                                            d.child(
                                                                div()
                                                                    .mt_1()
                                                                    .p_3()
                                                                    .rounded(px(6.))
                                                                    .bg(rgb(0xfafafa))
                                                                    .border_1()
                                                                    .border_color(rgb(0xe0e0e0))
                                                                    .child(
                                                                        div()
                                                                            .w_full()
                                                                            .text_xs()
                                                                            .text_color(rgb(0x333333))
                                                                            .child(current_summary)
                                                                    )
                                                            )
                                                        })
                                                )
                                        )
                                )
                            })
                    )
                    .child(
                        // 🎯 专业输入区域 - ChatGPT风格
                        div()
                            .w_full()
                            .p_4()
                            .border_t_1()
                            .border_color(rgb(0xe5e5e5))
                            .bg(rgb(0xffffff))  // 白色背景
                            .flex_shrink_0()
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        // ✨ 使用 gpui-component 的专业 Input 组件
                                        // 支持：中文输入、鼠标选择、复制粘贴、Undo/Redo、多行输入等功能
                                        div()
                                            .flex_1()
                                            .max_h(px(200.))  // 最大高度
                                            .bg(rgb(0xffffff))  // 白色背景
                                            .rounded(px(12.))   // ChatGPT风格的圆角
                                            .border_1()
                                            .border_color(rgb(0xd1d5db))
                                            .shadow_sm()
                                            .px_3()  // 内边距
                                            .py_2()
                                            .child(
                                                div()
                                                    .bg(rgb(0xffffff))  // Input内部也设置白色背景
                                                    .child(
                                                        Input::new(&self.input_state)
                                                            .cleanable()  // 显示清空按钮
                                                    )
                                            )
                                    )
                                    .child(
                                        // Send button - ChatGPT 风格
                                        {
                                            let input_text = self.input_state.read(cx).value().to_string();
                                            let is_empty = input_text.trim().is_empty();

                                            div()
                                                .size(px(40.))  // 圆形按钮
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .rounded(px(20.))  // 完全圆形
                                                .when(is_empty || self.is_loading, |d| {
                                                    d.bg(rgb(0xd1d5db))  // 禁用状态灰色
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(rgb(0xffffff))
                                                                .child("↑")
                                                        )
                                                })
                                                .when(!is_empty && !self.is_loading, |d| {
                                                    d.bg(rgb(0x10a37f))  // ChatGPT 绿色
                                                        .cursor_pointer()
                                                        .hover(|style| style.bg(rgb(0x0d8a6a)))
                                                        .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                            this.send_message(cx);
                                                        }))
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .text_color(rgb(0xffffff))
                                                                .font_weight(gpui::FontWeight::BOLD)
                                                                .child("↑")
                                                        )
                                                })
                                        }
                                    )
                            )
                    )
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
