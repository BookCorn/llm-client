use gpui::{div, prelude::*, px, rems, rgb, Window};
use gpui_component::{h_flex, text::{TextView, TextViewStyle}, v_flex};

use crate::models::Message;

/// 创建自定义的 Markdown 样式
fn create_markdown_style() -> TextViewStyle {
    TextViewStyle::default()
        .paragraph_gap(rems(0.5))  // 段落间距
        .heading_font_size(|level, _base| {
            // 自定义标题字体大小
            match level {
                1 => px(28.),   // H1 最大
                2 => px(24.),   // H2
                3 => px(20.),   // H3
                4 => px(18.),   // H4
                5 => px(16.),   // H5
                6 => px(14.),   // H6
                _ => px(14.),
            }
        })
}

pub fn render_message_list<V: gpui::Render + 'static>(
    messages: &[Message],
    expanded_reasoning_messages: &std::collections::HashSet<i64>,
    window: &mut Window,
    cx: &mut gpui::Context<V>,
    on_toggle_reasoning: impl Fn(&mut V, i64, &mut Window, &mut gpui::Context<V>) + 'static + Copy,
) -> impl IntoElement {
    let mut message_elements = Vec::new();
    for msg in messages.iter() {
        message_elements.push(render_single_message(
            msg,
            expanded_reasoning_messages.contains(&msg.timestamp.timestamp()),
            on_toggle_reasoning,
            window,
            cx,
        ));
    }

    v_flex()
        .w_full()
        .gap_4()
        .children(message_elements)
}

fn render_single_message<V: gpui::Render + 'static, F>(
    message: &Message,
    is_expanded: bool,
    on_toggle_reasoning: F,
    window: &mut Window,
    cx: &mut gpui::Context<V>,
) -> impl IntoElement + use<V, F>
where
    F: Fn(&mut V, i64, &mut Window, &mut gpui::Context<V>) + 'static + Copy,
{
    let is_user = message.is_user();
    let msg_timestamp = message.timestamp.timestamp();

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
                            .max_w(px(600.))  // 最大宽度
                            .px_4()  // 水平内边距
                            .py_3()  // 垂直内边距
                            .rounded(px(18.))  // 圆角
                            .bg(rgb(0x007bff))
                            .text_color(rgb(0xffffff))
                            .text_sm()
                            .child(message.content.clone())
                    )
            } else {
                // Assistant response with Markdown
                v_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x1a1a1a))  // 更深的颜色
                            .child("Assistant")
                    )
                    // 推理摘要（如果有）
                    .when_some(message.reasoning_summary.as_ref(), |d, summary| {
                        d.child(
                            div()
                                .w_full()
                                .max_w(px(700.))
                                .p_3()
                                .rounded(px(8.))
                                .bg(rgb(0xf5f5f5))
                                .border_1()
                                .border_color(rgb(0xd0d0d0))
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            // 标题行 - 可点击展开/收起
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .justify_between()
                                                .cursor_pointer()
                                                .on_mouse_down(gpui::MouseButton::Left, cx.listener(move |this: &mut V, _, window, cx| {
                                                    on_toggle_reasoning(this, msg_timestamp, window, cx);
                                                }))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(gpui::FontWeight::BOLD)
                                                                .text_color(rgb(0x2d2d2d))  // 深色文字
                                                                .child(if is_expanded { "▼" } else { "▶" })
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                                .text_color(rgb(0x2d2d2d))  // 深色文字
                                                                .child("🧠 推理过程")
                                                        )
                                                )
                                                .when_some(message.reasoning_duration, |d, duration| {
                                                    d.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(0x555555))
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .child(format!("⏱️ {:.1}s", duration))
                                                    )
                                                })
                                        )
                                        .when(is_expanded, |d| {
                                            // 展开时显示完整推理摘要 - 使用 Markdown 渲染
                                            d.child(
                                                div()
                                                    .mt_1()
                                                    .p_3()
                                                    .rounded(px(6.))
                                                    .bg(rgb(0xfafafa))
                                                    .border_1()
                                                    .border_color(rgb(0xe0e0e0))
                                                    .max_w(px(700.))  // 限制最大宽度防止溢出
                                                    .child(
                                                        div()
                                                            .w_full()
                                                            .text_xs()
                                                            .text_color(rgb(0x333333))  // 深色文字
                                                            .child(
                                                                TextView::markdown(
                                                                    ("reasoning", message.timestamp.timestamp() as usize),
                                                                    summary,
                                                                    window,
                                                                    cx,
                                                                )
                                                                .style(create_markdown_style())
                                                                .text_color(rgb(0x333333))
                                                                .text_xs()
                                                            )
                                                    )
                                            )
                                        })
                                )
                        )
                    })
                    .child(
                        div()
                            .w_full()
                            .max_w(px(900.))  // 添加最大宽度确保换行
                            .text_color(rgb(0x1a1a1a))  // 深色文字
                            .child(
                                TextView::markdown(
                                    ("msg", message.timestamp.timestamp() as usize),
                                    &message.content,
                                    window,
                                    cx,
                                )
                                .style(create_markdown_style())
                                .text_color(rgb(0x1a1a1a))  // 深色文字
                            )
                    )
            }
        )
}
