use gpui::{div, prelude::*, px, rgb, Window};
use gpui_component::{h_flex, text::TextView, v_flex};

use crate::models::Message;

pub fn render_message_list<V: gpui::Render>(
    messages: &[Message],
    window: &mut Window,
    cx: &mut gpui::Context<V>,
) -> impl IntoElement {
    let mut message_elements = Vec::new();
    for msg in messages.iter() {
        message_elements.push(render_single_message(msg, window, cx));
    }

    v_flex()
        .w_full()
        .gap_4()
        .children(message_elements)
}

fn render_single_message<V: gpui::Render>(
    message: &Message,
    window: &mut Window,
    cx: &mut gpui::Context<V>,
) -> impl IntoElement + use<V> {
    let is_user = message.is_user();

    v_flex()
        .w_full()
        .gap_2()
        .child(
            // User message or label
            if is_user {
                h_flex()
                    .w_full()
                    .justify_end()
                    .child(
                        div()
                            .max_w(px(700.))
                            .p_4()
                            .rounded_lg()
                            .bg(rgb(0x007bff))
                            .text_color(rgb(0xffffff))
                            .child(message.content.clone())
                    )
            } else {
                // Assistant response with Markdown
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x666666))
                            .child("Assistant")
                    )
                    .child(
                        div()
                            .w_full()
                            .child(
                                TextView::markdown(
                                    ("msg", message.timestamp.timestamp() as usize),
                                    &message.content,
                                    window,
                                    cx,
                                )
                            )
                    )
            }
        )
}
