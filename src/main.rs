use anyhow::Result;
use gpui::{
    actions, App, Application, AssetSource, Axis, Bounds, Context, EventEmitter, Focusable,
    FocusHandle, SharedString, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb,
    size,
};
use gpui_component::{h_flex, v_flex, StyledExt};
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

// Chat Application
struct ChatApp {
    current_conversation: Conversation,
    conversations: Vec<Conversation>,
    storage: StorageService,
    openai: OpenAIService,
    input_text: String,
    focus_handle: FocusHandle,
    use_real_api: bool,
    is_loading: bool,
}

impl EventEmitter<()> for ChatApp {}

impl Focusable for ChatApp {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ChatApp {
    fn new(cx: &mut Context<Self>) -> Self {
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
        let use_real_api = std::env::var("OPENAI_API_KEY").is_ok();

        Self {
            current_conversation,
            conversations,
            storage,
            openai: OpenAIService::new(),
            input_text: String::new(),
            focus_handle: cx.focus_handle(),
            use_real_api,
            is_loading: false,
        }
    }

    fn create_demo_conversation() -> Conversation {
        let mut conv = Conversation::new();
        conv.title = "Demo: GPUI Chat Interface".to_string();

        conv.add_message(Message::new_user(
            "Hello! Can you tell me about this chat application?".to_string(),
        ));

        conv.add_message(Message::new_assistant(
            "Hello! This is a **prototype chat application** built with GPUI and Rust.\n\n\
            ## Current Features\n\n\
            - ✅ Multi-turn conversation support\n\
            - ✅ Conversation history management\n\
            - ✅ Auto-save to local storage\n\
            - ✅ **Markdown rendering** (like this!)\n\
            - ✅ Clean, modern UI layout\n\n\
            ## Coming Soon\n\n\
            - 🔄 Full text input support\n\
            - 🤖 OpenAI API integration\n\
            - ⚡ Streaming responses\n\
            - 🎨 Code syntax highlighting".to_string(),
        ));

        conv.add_message(Message::new_user(
            "What technologies are you using?".to_string(),
        ));

        conv.add_message(Message::new_assistant(
            "This application is built with:\n\n\
            ```rust\n\
            // Core UI Framework\n\
            gpui = \"0.2\"\n\n\
            // UI Components\n\
            gpui-component = { git = \"...\" }\n\n\
            // Data Serialization\n\
            serde = \"1.0\"\n\
            serde_json = \"1.0\"\n\
            ```\n\n\
            The codebase is now **modularized** into:\n\n\
            1. `models/` - Data structures\n\
            2. `services/` - Business logic\n\
            3. `ui/` - UI components\n\n\
            This makes the code more maintainable and easier to understand!".to_string(),
        ));

        conv
    }

    fn new_conversation(&mut self, cx: &mut Context<Self>) {
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
        self.input_text.clear();
        cx.notify();
    }

    fn load_conversation(&mut self, id: Uuid, cx: &mut Context<Self>) {
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
            self.input_text.clear();
            cx.notify();
        }
    }

    fn send_message(&mut self, cx: &mut Context<Self>) {
        if self.input_text.trim().is_empty() || self.is_loading {
            return;
        }

        let user_message = Message::new_user(self.input_text.clone());
        self.current_conversation.add_message(user_message);

        let user_content = self.input_text.clone();
        self.input_text.clear();

        // TODO: Implement async OpenAI API integration
        // The cx.spawn pattern in GPUI 0.2 requires specific type annotations
        // that need further investigation. For now, using mock responses.

        let mock_response = if self.use_real_api {
            format!(
                "**Note: Real API Integration Pending**\n\nYou said:\n> {}\n\n\
                OpenAI API integration is prepared but async handling in GPUI 0.2 needs refinement.\n\n\
                The OpenAI service is ready at `src/services/openai.rs`.",
                user_content
            )
        } else {
            format!(
                "**Mock AI Response**\n\nYou said:\n> {}\n\n\
                This is a simulated response. The application is running in **mock mode**.\n\n\
                ✨ Try the markdown rendering and conversation management features!",
                user_content
            )
        };

        let assistant_message = Message::new_assistant(mock_response);
        self.current_conversation.add_message(assistant_message);

        // Auto-save
        let _ = self.storage.save_conversation(&self.current_conversation);

        cx.notify();
    }

    fn handle_input(&mut self, text: &str, cx: &mut Context<Self>) {
        self.input_text.push_str(text);
        cx.notify();
    }

    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        self.input_text.pop();
        cx.notify();
    }
}

impl Render for ChatApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let messages = self.current_conversation.messages.clone();
        let current_id = self.current_conversation.id;

        h_flex()
            .size_full()
            .bg(rgb(0xffffff))
            .child(
                // Sidebar
                render_sidebar(
                    &self.conversations,
                    current_id,
                    |this: &mut Self, _, _, cx| {
                        this.new_conversation(cx);
                    },
                    |this: &mut Self, id, _, _, cx| {
                        this.load_conversation(id, cx);
                    },
                    cx,
                )
            )
            .child(
                // Main Chat Area
                v_flex()
                    .flex_1()
                    .h_full()
                    .child(
                        // Chat Header
                        h_flex()
                            .h(px(64.))
                            .w_full()
                            .px_6()
                            .items_center()
                            .justify_between()
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
                        // Messages Area
                        div()
                            .flex_1()
                            .w_full()
                            .p_6()
                            .when(messages.is_empty(), |d| {
                                d.flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_color(rgb(0x6c757d))
                                            .child("Start a new conversation...")
                                    )
                            })
                            .child(render_message_list(&messages, window, cx))
                            .scrollable(Axis::Vertical)
                    )
                    .child(
                        // Input Area
                        div()
                            .w_full()
                            .p_4()
                            .border_t_1()
                            .border_color(rgb(0xdee2e6))
                            .bg(rgb(0xf8f9fa))
                            .track_focus(&self.focus_handle)
                            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                                if event.keystroke.key == "enter" {
                                    this.send_message(cx);
                                } else if event.keystroke.key == "backspace" {
                                    this.handle_backspace(cx);
                                } else if let Some(key_char) = &event.keystroke.key_char {
                                    this.handle_input(key_char, cx);
                                }
                            }))
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_2()
                                    .child(
                                        div()
                                            .flex_1()
                                            .h(px(48.))
                                            .px_4()
                                            .flex()
                                            .items_center()
                                            .border_1()
                                            .border_color(rgb(0xced4da))
                                            .rounded_lg()
                                            .bg(rgb(0xffffff))
                                            .cursor_text()
                                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, window, _cx| {
                                                window.focus(&this.focus_handle);
                                            }))
                                            .when(!self.input_text.is_empty(), |d| {
                                                d.child(self.input_text.clone())
                                            })
                                            .when(self.input_text.is_empty(), |d| {
                                                d.text_color(rgb(0xadb5bd))
                                                    .child("Type your message here...")
                                            })
                                    )
                                    .child(
                                        div()
                                            .h(px(48.))
                                            .px_6()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_lg()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .when(self.input_text.trim().is_empty(), |d| {
                                                d.bg(rgb(0xe9ecef))
                                                    .text_color(rgb(0x6c757d))
                                            })
                                            .when(!self.input_text.trim().is_empty(), |d| {
                                                d.bg(rgb(0x007bff))
                                                    .text_color(rgb(0xffffff))
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(rgb(0x0056b3)))
                                                    .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                                                        this.send_message(cx);
                                                    }))
                                            })
                                            .child("Send")
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

            cx.open_window(options, |_window, cx| {
                cx.activate(false);
                cx.new(|cx| ChatApp::new(cx))
            })
            .unwrap();
        });
}
