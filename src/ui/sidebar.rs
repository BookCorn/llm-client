use gpui::{div, prelude::*, px, rgb, Axis, MouseButton};
use gpui_component::{h_flex, v_flex, StyledExt};
use uuid::Uuid;

use crate::models::Conversation;

pub fn render_sidebar<V: gpui::Render>(
    conversations: &[Conversation],
    current_id: Uuid,
    on_new: impl Fn(&mut V, &gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::Context<V>) + 'static,
    on_select: impl Fn(&mut V, Uuid, &gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::Context<V>) + Clone + 'static,
    cx: &mut gpui::Context<V>,
) -> impl IntoElement {
    v_flex()
        .w(px(280.))
        .h_full()
        .bg(rgb(0xf8f9fa))
        .border_r_1()
        .border_color(rgb(0xdee2e6))
        .child(render_sidebar_header(on_new, cx))
        .child(render_conversations_list(conversations, current_id, on_select, cx))
}

fn render_sidebar_header<V: gpui::Render>(
    on_new: impl Fn(&mut V, &gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::Context<V>) + 'static,
    cx: &mut gpui::Context<V>,
) -> impl IntoElement {
    h_flex()
        .h(px(64.))
        .w_full()
        .px_4()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(rgb(0xdee2e6))
        .bg(rgb(0xffffff))
        .child(
            div()
                .text_lg()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0x212529))
                .child("Conversations")
        )
        .child(
            div()
                .w(px(36.))
                .h(px(36.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_lg()
                .bg(rgb(0x007bff))
                .text_color(rgb(0xffffff))
                .font_weight(gpui::FontWeight::BOLD)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x0056b3)))
                .on_mouse_down(MouseButton::Left, cx.listener(on_new))
                .child("+")
        )
}

fn render_conversations_list<V: gpui::Render>(
    conversations: &[Conversation],
    current_id: Uuid,
    on_select: impl Fn(&mut V, Uuid, &gpui::MouseDownEvent, &mut gpui::Window, &mut gpui::Context<V>) + Clone + 'static,
    cx: &mut gpui::Context<V>,
) -> impl IntoElement {
    div()
        .flex_1()
        .w_full()
        .children(
            conversations.iter().map(|conv| {
                let conv_id = conv.id;
                let is_active = conv_id == current_id;
                let on_select = on_select.clone();

                div()
                    .w_full()
                    .p_3()
                    .border_b_1()
                    .border_color(rgb(0xdee2e6))
                    .cursor_pointer()
                    .when(is_active, |d| d.bg(rgb(0xe7f1ff)))
                    .when(!is_active, |d| {
                        d.hover(|style| style.bg(rgb(0xf8f9fa)))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |view, event, window, cx| {
                        on_select(view, conv_id, event, window, cx);
                    }))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(0x212529))
                                    .child(conv.title.clone())
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x6c757d))
                                    .child(format!("{} messages", conv.messages.len()))
                            )
                    )
            })
        )
        .scrollable(Axis::Vertical)
}
