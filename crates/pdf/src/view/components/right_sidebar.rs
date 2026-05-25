use crate::view::PdfReaderView;
use crate::view::types::{PdfIconName, RightSidebarTab};
use gpui::prelude::*;
use gpui::{ClipboardItem, Context, MouseButton, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::scroll::ScrollableElement;
use gpui_component::text::TextView;
use gpui_component::{ActiveTheme, Icon, Selectable, h_flex, label::Label, v_flex};
use i18n::I18nKey;

impl PdfReaderView {
    pub(crate) fn render_right_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .w(self.right_sidebar_width)
            .flex_shrink_0()
            .overflow_hidden()
            .h_full()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                h_flex()
                    .w_full()
                    .h_9()
                    .border_b_1()
                    .border_color(theme.border)
                    .px_1()
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new("right-tab-translation")
                            .ghost()
                            .icon(PdfIconName::Translate)
                            .when(
                                self.active_right_sidebar_tab == RightSidebarTab::Translation,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_right_sidebar_tab = RightSidebarTab::Translation;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("right-tab-notes")
                            .ghost()
                            .icon(PdfIconName::FileText)
                            .when(
                                self.active_right_sidebar_tab == RightSidebarTab::Notes,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_right_sidebar_tab = RightSidebarTab::Notes;
                                cx.notify();
                            })),
                    ),
            )
            .child(v_flex().flex_grow().h_0().w_full().child({
                let element: gpui::AnyElement = match self.active_right_sidebar_tab {
                    RightSidebarTab::Translation => v_flex()
                        .size_full()
                        .child(self.render_translation_content(cx))
                        .child(self.render_translation_bottom_bar(cx))
                        .into_any_element(),
                    RightSidebarTab::Notes => {
                        self.render_notes_content(window, cx).into_any_element()
                    }
                };
                element
            }))
    }

    fn render_translation_content(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let result = &self.translation_result;

        let original_text = result
            .as_ref()
            .map(|r| r.original.clone())
            .unwrap_or_default();
        let original_for_copy = original_text.clone();
        let is_placeholder = original_text.is_empty();
        let original_display: gpui::SharedString = if is_placeholder {
            i18n::t(I18nKey::SelectTextToTranslate, self.language).into()
        } else {
            original_text.into()
        };

        let translated_text = result.as_ref().and_then(|r| r.translated.clone());

        v_flex()
            .w_full()
            .p_3()
            .gap_2()
            .h_full()
            .relative() // 重要：为绝对定位的菜单提供容器
            .child(
                // ── 原文区域（可折叠）──
                v_flex()
                    .w_full()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .when(self.translation_original_expanded, |this| {
                        this.flex_grow().h_0()
                    })
                    .child(
                        h_flex()
                            .px_2()
                            .py_1()
                            .bg(theme.accent.opacity(0.05))
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.translation_original_expanded =
                                                !this.translation_original_expanded;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Icon::new(if self.translation_original_expanded {
                                            PdfIconName::ChevronDown
                                        } else {
                                            PdfIconName::ChevronRight
                                        })
                                        .text_color(theme.muted_foreground),
                                    )
                                    .child(
                                        Label::new(i18n::t(
                                            I18nKey::OriginalSection,
                                            self.language,
                                        ))
                                        .text_sm()
                                        .text_color(theme.foreground),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        div()
                                            .relative()
                                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                                if this.is_engine_menu_open {
                                                    this.is_engine_menu_open = false;
                                                    cx.notify();
                                                }
                                            }))
                                            .child(
                                                Button::new("engine-selector")
                                                    .ghost()
                                                    .icon(PdfIconName::Translate)
                                                    .compact()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.is_engine_menu_open =
                                                            !this.is_engine_menu_open;
                                                        cx.notify();
                                                    })),
                                            ),
                                    )
                                    .when(!original_for_copy.is_empty(), |this| {
                                        this.child(
                                            Button::new("copy-original")
                                                .ghost()
                                                .icon(PdfIconName::ClipboardCopy)
                                                .compact()
                                                .on_click(cx.listener(
                                                    move |_this, _, _window, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(
                                                                original_for_copy.clone(),
                                                            ),
                                                        );
                                                    },
                                                )),
                                        )
                                    }),
                            ),
                    )
                    .when(self.translation_original_expanded, |this| {
                        this.child(
                            v_flex()
                                .flex_grow()
                                .h_0()
                                .w_full()
                                .overflow_y_scrollbar()
                                .px_2()
                                .py_2()
                                .child(
                                    Label::new(original_display.clone())
                                        .w_full()
                                        .text_size(px(self.translation_font_size))
                                        .when(is_placeholder, |l| {
                                            l.text_color(theme.muted_foreground)
                                        }),
                                ),
                        )
                    }),
            )
            .child(
                // ── 译文区域（始终撑满剩余空间）──
                v_flex()
                    .w_full()
                    .flex_grow()
                    .h_0()
                    .border_1()
                    .border_color(theme.border)
                    .rounded_md()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .px_2()
                            .py_1()
                            .bg(theme.accent.opacity(0.05))
                            .items_center()
                            .justify_between()
                            .child(
                                Label::new(i18n::t(I18nKey::TranslationSection, self.language))
                                    .text_sm()
                                    .text_color(theme.foreground),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .when_some(self.translation_result.clone(), |this, res| {
                                        this.child(
                                            Button::new("retry-translation")
                                                .ghost()
                                                .icon(PdfIconName::RotateCw)
                                                .compact()
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    this.translate_text(res.original.clone(), cx);
                                                })),
                                        )
                                    })
                                    .when_some(translated_text.clone(), |this, text| {
                                        this.child(
                                            Button::new("copy-translated")
                                                .ghost()
                                                .icon(PdfIconName::ClipboardCopy)
                                                .compact()
                                                .on_click(cx.listener(
                                                    move |_this, _, _window, cx| {
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(text.clone()),
                                                        );
                                                    },
                                                )),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_grow()
                            .h_0()
                            .w_full()
                            .overflow_y_scrollbar()
                            .px_2()
                            .py_2()
                            .child({
                                let (text, color): (gpui::SharedString, gpui::Hsla) = match result {
                                    Some(res) if res.is_loading => (
                                        i18n::t(I18nKey::Translating, self.language).into(),
                                        theme.muted_foreground,
                                    ),
                                    Some(res) if res.error.is_some() => {
                                        (res.error.clone().unwrap().into(), gpui::red())
                                    }
                                    Some(res) => match &res.translated {
                                        Some(t) => (t.clone().into(), theme.foreground),
                                        None => (
                                            i18n::t(I18nKey::TranslationPending, self.language)
                                                .into(),
                                            theme.muted_foreground,
                                        ),
                                    },
                                    None => (
                                        i18n::t(I18nKey::SelectTextToTranslate, self.language)
                                            .into(),
                                        theme.muted_foreground,
                                    ),
                                };
                                Label::new(text)
                                    .w_full()
                                    .text_size(px(self.translation_font_size))
                                    .text_color(color)
                                    .into_any_element()
                            }),
                    ),
            )
            .when(self.is_engine_menu_open, |this| {
                let current_engine_id = self
                    .delegate
                    .as_ref()
                    .map(|d| d.current_translation_engine_id())
                    .unwrap_or_default();

                this.child(
                    v_flex()
                        .absolute()
                        .top(px(40.0))
                        .right(px(16.0))
                        .w(px(140.0))
                        .bg(theme.popover)
                        .border_1()
                        .border_color(theme.border)
                        .shadow_lg()
                        .rounded_md()
                        .p_1()
                        .occlude()
                        .children(
                            self.delegate
                                .as_ref()
                                .map(|d| d.get_translation_engines())
                                .unwrap_or_default()
                                .into_iter()
                                .enumerate()
                                .map(|(idx, name)| {
                                    let name_clone = name.clone();
                                    let is_active = name == current_engine_id;
                                    let item_display = match name.as_str() {
                                        "google_free" => "Google (Free)",
                                        "bing_free" => "Bing (Free)",
                                        "google" => "Google Cloud",
                                        "niutrans" => i18n::t(I18nKey::NiuTrans, self.language),
                                        _ => name.as_str(),
                                    };
                                    h_flex()
                                        .id(("engine-item", idx))
                                        .p_1()
                                        .px_2()
                                        .gap_2()
                                        .rounded_sm()
                                        .hover(|s| s.bg(theme.accent.opacity(0.1)))
                                        .child(h_flex().w_4().items_center().justify_center().when(
                                            is_active,
                                            |this| {
                                                this.child(
                                                    Icon::new(PdfIconName::Check)
                                                        .size(px(12.0))
                                                        .text_color(theme.primary),
                                                )
                                            },
                                        ))
                                        .child(Label::new(item_display.to_string()).text_xs())
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if let Some(delegate) = &this.delegate {
                                                delegate.set_translation_engine(name_clone.clone());
                                            }
                                            this.is_engine_menu_open = false;
                                            cx.notify();
                                        }))
                                }),
                        ),
                )
            })
    }

    fn render_notes_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let notes = self
            .delegate
            .as_ref()
            .and_then(|d| d.get_notes(&self.document_id))
            .unwrap_or_default();

        let theme = cx.theme();
        let muted = theme.muted_foreground;

        v_flex()
            .size_full()
            .child(
                // ── 笔记顶部 Header ──
                h_flex()
                    .w_full()
                    .px_3()
                    .py_2()
                    .items_center()
                    .justify_between()
                    .child(
                        Label::new(i18n::t(I18nKey::Notes, self.language))
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(muted),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .when(self.notes_edit_mode, |this| {
                                this.child(
                                    Button::new("notes-cancel")
                                        .ghost()
                                        .label(i18n::t(I18nKey::Cancel, self.language))
                                        .compact()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.notes_edit_mode = false;
                                                cx.notify();
                                            }),
                                        ),
                                )
                                .child(
                                    Button::new("notes-save")
                                        .label(i18n::t(I18nKey::Save, self.language))
                                        .compact()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                if let Some(input) = &this.notes_input_state {
                                                    let text = input.read(cx).text().to_string();
                                                    if let Some(delegate) = &this.delegate {
                                                        delegate
                                                            .save_notes(&this.document_id, &text);
                                                    }
                                                }
                                                this.notes_edit_mode = false;
                                                this.notes_input_state = None;
                                                cx.notify();
                                            }),
                                        ),
                                )
                            })
                            .when(!self.notes_edit_mode, |this| {
                                this.child(
                                    Button::new("notes-edit")
                                        .ghost()
                                        .label(i18n::t(I18nKey::Edit, self.language))
                                        .compact()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.notes_edit_mode = true;
                                            this.notes_input_state = None;
                                            cx.notify();
                                        })),
                                )
                            }),
                    ),
            )
            .child(v_flex().flex_grow().h_0().child({
                if self.notes_edit_mode {
                    if self.notes_input_state.is_none() {
                        let entity = cx.new(|cx| InputState::new(window, cx).multi_line(true));
                        entity.update(cx, |state, cx| {
                            state.set_value(&notes, window, cx);
                        });
                        self.notes_input_state = Some(entity);
                    }
                    if let Some(input) = &self.notes_input_state {
                        v_flex()
                            .px_3()
                            .pb_3()
                            .size_full()
                            .child(Input::new(input).w_full().h_full())
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }
                } else {
                    if notes.is_empty() {
                        v_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .child(
                                Label::new(i18n::t(I18nKey::NoNotes, self.language))
                                    .text_color(muted),
                            )
                            .into_any_element()
                    } else {
                        v_flex()
                            .px_3()
                            .pb_3()
                            .size_full()
                            .child(
                                TextView::markdown("notes-view", &notes, window, cx)
                                    .selectable(true),
                            )
                            .into_any_element()
                    }
                }
            }))
    }

    fn render_translation_bottom_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .h_10()
            .px_3()
            .items_center()
            .justify_center()
            .border_t_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .bg(theme.muted.opacity(0.2))
                    .rounded_md()
                    .items_center()
                    .child(
                        Button::new("font-size-decrease")
                            .ghost()
                            .icon(PdfIconName::ZoomOut)
                            .compact()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.change_translation_font_size(-1.0, cx);
                            })),
                    )
                    .child(
                        div().px_1().min_w(px(24.0)).child(
                            Label::new(format!("{}", self.translation_font_size as i32))
                                .text_xs()
                                .text_center()
                                .text_color(theme.muted_foreground),
                        ),
                    )
                    .child(
                        Button::new("font-size-increase")
                            .ghost()
                            .icon(PdfIconName::ZoomIn)
                            .compact()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.change_translation_font_size(1.0, cx);
                            })),
                    ),
            )
    }
}
