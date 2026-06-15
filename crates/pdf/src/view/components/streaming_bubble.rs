use crate::view::types::PdfIconName;
use gpui::prelude::*;
use gpui::{Context, Window, div, px, relative};
use gpui_component::text::{TextView, TextViewStyle};
use gpui_component::{ActiveTheme, Icon, h_flex, label::Label, v_flex};
use i18n::{I18nKey, Language};
use crate::view::components::chat_session_view::split_markdown_blocks;

pub(crate) const CHAT_BODY_FONT_SIZE: gpui::Pixels = px(14.);

pub(crate) struct StreamingBubbleView {
    pub(crate) text: String,
    pub(crate) reasoning: String,
    reasoning_expanded: bool,
    language: Language,
}

impl StreamingBubbleView {
    pub fn new(language: Language, _cx: &mut Context<Self>) -> Self {
        Self {
            text: String::new(),
            reasoning: String::new(),
            reasoning_expanded: false,
            language,
        }
    }

    pub fn append_content(&mut self, s: &str, cx: &mut Context<Self>) {
        self.text.push_str(s);
        cx.notify();
    }

    pub fn append_reasoning(&mut self, s: &str, cx: &mut Context<Self>) {
        self.reasoning.push_str(s);
        cx.notify();
    }

    pub fn take_final(&mut self) -> (String, String) {
        (
            std::mem::take(&mut self.text),
            std::mem::take(&mut self.reasoning),
        )
    }
}

impl gpui::Render for StreamingBubbleView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme().clone();
        let bubble_color = theme.muted;

        let reasoning = if self.reasoning.is_empty() {
            None
        } else {
            Some(self.reasoning.as_str())
        };

        let display_content = if self.text.is_empty() {
            if self.reasoning.is_empty() {
                i18n::t(I18nKey::AiThinking, self.language).to_string()
            } else {
                String::new()
            }
        } else {
            self.text.clone()
        };

        let cursor = format!("{} ▊", &display_content);

        v_flex().w_full().items_start().child(
            v_flex()
                .w(relative(0.8))
                .bg(bubble_color.opacity(0.15))
                .rounded_md()
                .px_2()
                .py_1()
                .gap_0p5()
                .when_some(reasoning, |this, r| {
                    let is_expanded = self.reasoning_expanded
                        || (!self.reasoning.is_empty() && self.text.is_empty());
                    this.child(
                        v_flex()
                            .w_full()
                            .my_1()
                            .child(
                                h_flex()
                                    .id("reasoning-toggle-streaming")
                                    .cursor_pointer()
                                    .items_center()
                                    .gap_1()
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.reasoning_expanded = !this.reasoning_expanded;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Icon::new(if is_expanded {
                                            PdfIconName::ChevronDown
                                        } else {
                                            PdfIconName::ChevronRight
                                        })
                                        .size(px(12.0))
                                        .text_color(theme.muted_foreground),
                                    )
                                    .child(
                                        Label::new(if !self.text.is_empty() {
                                            "已思考"
                                        } else {
                                            "思考中..."
                                        })
                                        .text_xs()
                                        .text_color(theme.muted_foreground),
                                    ),
                            )
                            .when(is_expanded, |this| {
                                this.child(
                                    h_flex()
                                        .pl_2()
                                        .border_l_1()
                                        .border_color(theme.muted_foreground.opacity(0.3))
                                        .child(
                                            Label::new(r.to_string())
                                                .text_xs()
                                                .text_color(theme.muted_foreground.opacity(0.8)),
                                        ),
                                )
                            }),
                    )
                })
                .child(
                    div().relative().child({
                        let blocks = split_markdown_blocks(&cursor);
                        let mut blocks_container = v_flex().gap_2();
                        for (block_idx, block_text) in blocks.into_iter().enumerate() {
                            blocks_container = blocks_container.child(
                                TextView::markdown(
                                    gpui::SharedString::from(format!("chat-msg-streaming-b{}", block_idx)),
                                    gpui::SharedString::from(block_text),
                                    window,
                                    cx,
                                )
                                .style(
                                    TextViewStyle::default().heading_font_size(|level, _| match level {
                                        1 => CHAT_BODY_FONT_SIZE + px(8.),
                                        2 => CHAT_BODY_FONT_SIZE + px(6.),
                                        3 => CHAT_BODY_FONT_SIZE + px(4.),
                                        4 => CHAT_BODY_FONT_SIZE + px(2.),
                                        _ => CHAT_BODY_FONT_SIZE + px(1.),
                                    }),
                                )
                                .selectable(true)
                                .text_size(CHAT_BODY_FONT_SIZE)
                                .text_color(theme.foreground)
                            );
                        }
                        blocks_container
                    }),
                ),
        )
    }
}
