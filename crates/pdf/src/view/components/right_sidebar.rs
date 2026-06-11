use crate::view::PdfReaderView;
use crate::view::components::chat_session_view::ChatSessionView;
use crate::view::types::{PdfIconName, RightSidebarTab};
use gpui::prelude::*;
use gpui::{ClipboardItem, Context, Window, div, px};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;
use gpui_component::select::Select;
use gpui_component::text::TextView;
use gpui_component::{ActiveTheme, Icon, Selectable, h_flex, label::Label, v_flex};
use i18n::I18nKey;
use log::debug;

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
                    )
                    .child(
                        Button::new("right-tab-chat")
                            .ghost()
                            .icon(PdfIconName::MessageSquare)
                            .when(
                                self.active_right_sidebar_tab == RightSidebarTab::Chat,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_right_sidebar_tab = RightSidebarTab::Chat;
                                cx.notify();
                            })),
                    ),
            )
            .child(v_flex().flex_grow().h_0().w_full().child({
                let element: gpui::AnyElement = match self.active_right_sidebar_tab {
                    RightSidebarTab::Translation => v_flex()
                        .size_full()
                        .child(self.render_translation_content(window, cx))
                        .child(self.render_translation_bottom_bar(cx))
                        .into_any_element(),
                    RightSidebarTab::Notes => {
                        self.render_notes_content(window, cx).into_any_element()
                    }
                    RightSidebarTab::Chat => {
                        self.render_chat_content(window, cx).into_any_element()
                    }
                };
                element
            }))
    }

    fn render_translation_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let select_state = self.get_or_create_engine_select(window, cx);
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
                                            if let Some(ref delegate) = this.delegate {
                                                delegate.set_translation_original_expanded(
                                                    this.translation_original_expanded,
                                                );
                                            }
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
                                    .child(div().w(px(140.0)).child(Select::new(&select_state)))
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
                                    .child(
                                        Button::new("auto-translate-toggle")
                                            .ghost()
                                            .icon(PdfIconName::FastForward)
                                            .compact()
                                            .text_color(if self.auto_translate {
                                                theme.primary
                                            } else {
                                                theme.muted_foreground
                                            })
                                            .tooltip(i18n::t(
                                                if self.auto_translate {
                                                    I18nKey::AutoTranslateOn
                                                } else {
                                                    I18nKey::AutoTranslateOff
                                                },
                                                self.language,
                                            ))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.auto_translate = !this.auto_translate;
                                                this.save_current_state();
                                                cx.notify();
                                            })),
                                    )
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
    }

    fn render_notes_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;

        // 首次渲染时加载笔记
        if self.notes_cache.is_empty() {
            if let Some(delegate) = &self.delegate {
                let lit_id = self
                    .document_id
                    .split("::")
                    .next()
                    .unwrap_or(&self.document_id);
                self.notes_cache = delegate.list_notes(lit_id);
            }
        }

        if let Some(index) = self.editing_note_index {
            // 确保输入框状态在新建/编辑时都被正确初始化
            if self.edit_note_title.is_none() || self.edit_note_content.is_none() {
                let note = &self.notes_cache[index];
                let title = note.title.clone();
                let content = note.content.clone();

                let entity = cx.new(|cx| {
                    gpui_component::input::InputState::new(window, cx).placeholder("输入标题...")
                });
                entity.update(cx, |s, cx| {
                    s.set_value(&title, window, cx);
                });
                self.edit_note_title = Some(entity);

                let entity2 = cx.new(|cx| {
                    gpui_component::input::InputState::new(window, cx)
                        .multi_line(true)
                        .placeholder("输入内容 (支持 Markdown)...")
                });
                entity2.update(cx, |s, cx| {
                    s.set_value(&content, window, cx);
                });
                self.edit_note_content = Some(entity2);
            }

            let note = &self.notes_cache[index];
            let note_id = note.id.clone();

            return v_flex()
                .size_full()
                .p_3()
                .gap_3()
                .child(
                    // ── 顶部栏：包含标题和操作按钮 ──
                    h_flex()
                        .w_full()
                        .justify_between()
                        .items_center()
                        .child(
                            Label::new("编辑笔记")
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(muted),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Button::new(gpui::SharedString::from(format!(
                                        "note-cancel-{index}"
                                    )))
                                    .ghost()
                                    .icon(PdfIconName::Close)
                                    .compact()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if this.notes_cache.get(index).map(|n| n.id.as_str())
                                            == Some("temp_new_note")
                                        {
                                            this.notes_cache.remove(index);
                                        }
                                        this.editing_note_index = None;
                                        this.edit_note_title = None;
                                        this.edit_note_content = None;
                                        cx.notify();
                                    })),
                                )
                                .child(
                                    Button::new(gpui::SharedString::from(format!(
                                        "note-save-{index}"
                                    )))
                                    .ghost()
                                    .icon(PdfIconName::Check)
                                    .compact()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let new_title = this
                                            .edit_note_title
                                            .as_ref()
                                            .map(|e| e.read(cx).text().to_string());
                                        let new_content = this
                                            .edit_note_content
                                            .as_ref()
                                            .map(|e| e.read(cx).text().to_string());

                                        let mut final_note_id = note_id.clone();
                                        let is_temp = note_id == "temp_new_note";

                                        if is_temp {
                                            if let Some(delegate) = &this.delegate {
                                                let lit_id = this
                                                    .document_id
                                                    .split("::")
                                                    .next()
                                                    .unwrap_or(&this.document_id)
                                                    .to_string();
                                                let default_title = new_title
                                                    .clone()
                                                    .unwrap_or_else(|| "未命名笔记".to_string());
                                                if let Some(real_id) =
                                                    delegate.create_note(&lit_id, &default_title)
                                                {
                                                    final_note_id = real_id;
                                                }
                                            }
                                        }

                                        if let Some(delegate) = &this.delegate {
                                            delegate.update_note(
                                                &final_note_id,
                                                new_title.as_deref(),
                                                new_content.as_deref(),
                                            );
                                        }
                                        if let Some(note) = this.notes_cache.get_mut(index) {
                                            note.id = final_note_id;
                                            if let Some(ref t) = new_title {
                                                note.title = t.clone();
                                            }
                                            if let Some(ref c) = new_content {
                                                note.content = c.clone();
                                            }
                                        }
                                        this.editing_note_index = None;
                                        this.edit_note_title = None;
                                        this.edit_note_content = None;
                                        cx.notify();
                                    })),
                                ),
                        ),
                )
                .when_some(self.edit_note_title.as_ref(), |this, e| {
                    this.child(gpui_component::input::Input::new(e).w_full())
                })
                .child(
                    // ── 内容输入框，通过 div 容器包裹撑满剩余纵向空间 ──
                    div().w_full().flex_grow().h_0().when_some(
                        self.edit_note_content.as_ref(),
                        |this, e| {
                            this.child(gpui_component::input::Input::new(e).w_full().h_full())
                        },
                    ),
                )
                .into_any_element();
        }

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
                        Button::new("add-note")
                            .ghost()
                            .icon(PdfIconName::ZoomIn)
                            .compact()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let lit_id = this
                                    .document_id
                                    .split("::")
                                    .next()
                                    .unwrap_or(&this.document_id)
                                    .to_string();
                                let title = "".to_string();
                                let note = models::LiteratureNote {
                                    id: "temp_new_note".to_string(),
                                    literature_id: lit_id,
                                    title,
                                    content: String::new(),
                                    sort_order: this.notes_cache.len() as i32,
                                    created_at: chrono::Utc::now().timestamp(),
                                    updated_at: chrono::Utc::now().timestamp(),
                                };
                                this.notes_cache.push(note);
                                this.editing_note_index = Some(this.notes_cache.len() - 1);
                                this.edit_note_title = None;
                                this.edit_note_content = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_grow()
                    .h_0()
                    .overflow_y_scrollbar()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .children({
                        let mut cards: Vec<gpui::AnyElement> = Vec::new();
                        let notes_snapshot = self.notes_cache.clone();
                        for (i, note) in notes_snapshot.iter().enumerate() {
                            let card = self.render_note_card(i, note, window, cx);
                            cards.push(card.into_any_element());
                        }
                        if self.notes_cache.is_empty() {
                            let empty_lang = self.language;
                            cards.push(
                                v_flex()
                                    .size_full()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Label::new(i18n::t(I18nKey::NoNotes, empty_lang))
                                            .text_xs()
                                            .text_color(muted),
                                    )
                                    .into_any_element(),
                            );
                        }
                        cards
                    }),
            )
            .into_any_element()
    }

    fn render_note_card(
        &mut self,
        index: usize,
        note: &models::LiteratureNote,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let border_color = theme.border;
        let accent_color = theme.accent;
        let muted_color = theme.muted;
        let muted_foreground = theme.muted_foreground;
        let note_id = note.id.clone();

        let local_time = chrono::DateTime::from_timestamp(note.updated_at, 0)
            .map(|dt| dt.with_timezone(&chrono::Local))
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();

        v_flex()
            .w_full()
            .group("note-card")
            .bg(muted_color.opacity(0.3))
            .border_1()
            .border_color(border_color)
            .rounded_md()
            .overflow_hidden()
            .hover(|s| s.border_color(accent_color))
            .child(
                // ── 标题栏：带轻微背景色与分隔线 ──
                h_flex()
                    .w_full()
                    .bg(muted_color.opacity(0.12))
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(border_color)
                    .justify_between()
                    .items_center()
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(note.title.clone())
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis(),
                        ),
                    )
                    .child(
                        // 按钮组：默认不可见，Hover 卡片时显现
                        h_flex()
                            .gap_0()
                            .opacity(0.0)
                            .group_hover("note-card", |s| s.opacity(1.0))
                            .child(
                                Button::new(gpui::SharedString::from(format!("note-edit-{index}")))
                                    .ghost()
                                    .icon(PdfIconName::Annotations)
                                    .compact()
                                    .on_click({
                                        let et = note.title.clone();
                                        let ec = note.content.clone();
                                        cx.listener(move |this, _, window, cx| {
                                            this.editing_note_index = Some(index);
                                            let entity = cx.new(|cx| {
                                                gpui_component::input::InputState::new(window, cx)
                                                    .placeholder("标题")
                                            });
                                            entity.update(cx, |s, cx| {
                                                s.set_value(&et, window, cx);
                                            });
                                            this.edit_note_title = Some(entity);
                                            let entity2 = cx.new(|cx| {
                                                gpui_component::input::InputState::new(window, cx)
                                                    .multi_line(true)
                                            });
                                            entity2.update(cx, |s, cx| {
                                                s.set_value(&ec, window, cx);
                                            });
                                            this.edit_note_content = Some(entity2);
                                            cx.notify();
                                        })
                                    }),
                            )
                            .child(
                                Button::new(gpui::SharedString::from(format!(
                                    "note-delete-{index}"
                                )))
                                .ghost()
                                .icon(PdfIconName::Close)
                                .compact()
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some(delegate) = &this.delegate {
                                            delegate.delete_note(&note_id);
                                        }
                                        this.notes_cache.retain(|n| n.id != note_id);
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                // ── 内容及时间戳区域 ──
                v_flex()
                    .p_2()
                    .gap_1p5()
                    .child(
                        TextView::markdown(
                            gpui::SharedString::from(format!("note-content-{index}")),
                            &note.content,
                            window,
                            cx,
                        )
                        .selectable(true)
                        .text_xs(),
                    )
                    .child(
                        h_flex().justify_end().child(
                            Label::new(local_time)
                                .text_xs()
                                .text_color(muted_foreground),
                        ),
                    ),
            )
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

    // ── AI 对话 ─────────────────────────────────────────

    fn render_chat_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let _muted = theme.muted_foreground;

        if self.chat_sessions.is_empty() {
            if let Some(delegate) = &self.delegate {
                let lit_id = self
                    .document_id
                    .split("::")
                    .next()
                    .unwrap_or(&self.document_id);
                debug!("[Chat] render_chat_content: loading sessions from DB, lit_id={lit_id}");
                self.chat_sessions = delegate.list_chat_sessions(lit_id);
                debug!(
                    "[Chat] render_chat_content: loaded {} sessions",
                    self.chat_sessions.len()
                );
            }
        }

        debug!(
            "[Chat] render_chat_content: sessions={}, creating={}, active={:?}",
            self.chat_sessions.len(),
            self.chat_creating,
            self.active_chat_session_id,
        );

        if self.chat_creating {
            return self.render_chat_create_form(window, cx).into_any_element();
        }

        if let Some(session_id) = &self.active_chat_session_id.clone() {
            if self.chat_session_view.is_none() {
                if let Some(delegate) = &self.delegate {
                    let messages = delegate.list_chat_messages(session_id);
                    let session = self
                        .chat_sessions
                        .iter()
                        .find(|s| s.id == *session_id)
                        .cloned();
                    if let Some(s) = session {
                        let parent_handle = cx.entity().downgrade();
                        let entity = cx.new(|cx| {
                            ChatSessionView::new(
                                self.delegate.clone(),
                                self.language,
                                session_id.clone(),
                                s.title.clone(),
                                s.system_prompt.clone(),
                                messages,
                                parent_handle,
                                cx,
                            )
                        });
                        self.chat_session_view = Some(entity);
                    }
                }
            }
            if let Some(ref view) = self.chat_session_view {
                return view.clone().into_any_element();
            }
            self.render_chat_session_list(window, cx).into_any_element()
        } else {
            self.render_chat_session_list(window, cx).into_any_element()
        }
    }

    fn render_chat_create_form(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        if self.chat_create_title.is_none() {
            let entity = cx.new(|cx| {
                gpui_component::input::InputState::new(window, cx).placeholder("对话标题")
            });
            entity.update(cx, |s, cx| {
                s.set_value(
                    i18n::t(I18nKey::DefaultChatTitle, self.language),
                    window,
                    cx,
                );
            });
            self.chat_create_title = Some(entity);

            let entity2 = cx.new(|cx| {
                gpui_component::input::InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder("系统提示词 (可选)")
            });
            entity2.update(cx, |s, cx| {
                s.set_value(
                    "You are a knowledgeable research assistant helping the user analyze an academic paper. Answer questions about the content, explain concepts, and provide insights based on the paper text.",
                    window,
                    cx,
                );
            });
            self.chat_create_prompt = Some(entity2);
        }

        v_flex()
            .size_full()
            .p_3()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        Label::new(i18n::t(I18nKey::NewChat, self.language))
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(muted),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("chat-create-cancel")
                                .ghost()
                                .icon(PdfIconName::Close)
                                .compact()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.chat_creating = false;
                                    this.chat_create_title = None;
                                    this.chat_create_prompt = None;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("chat-create-confirm")
                                .ghost()
                                .icon(PdfIconName::Check)
                                .compact()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let title = this
                                        .chat_create_title
                                        .as_ref()
                                        .map(|e| e.read(cx).text().to_string())
                                        .unwrap_or_default();
                                    let prompt = this
                                        .chat_create_prompt
                                        .as_ref()
                                        .map(|e| e.read(cx).text().to_string())
                                        .unwrap_or_default();
                                    let lit_id = this
                                        .document_id
                                        .split("::")
                                        .next()
                                        .unwrap_or(&this.document_id)
                                        .to_string();
                                    debug!("[Chat] create confirm: title={title:?}, lit_id={lit_id}, prompt_len={}", prompt.len());
                                    if let Some(delegate) = &this.delegate {
                                        if let Some(id) =
                                            delegate.create_chat_session(&lit_id, &title, &prompt)
                                        {
                                            debug!("[Chat] create confirm: OK, session_id={id}");
                                            this.active_chat_session_id = Some(id.clone());
                                            let session = models::chat::ChatSession {
                                                id,
                                                literature_id: lit_id,
                                                title,
                                                system_prompt: prompt,
                                                created_at: chrono::Utc::now().timestamp(),
                                                updated_at: chrono::Utc::now().timestamp(),
                                            };
                                            this.chat_sessions.insert(0, session);
                                            debug!("[Chat] create confirm: inserted into chat_sessions, len={}", this.chat_sessions.len());
                                        } else {
                                            debug!("[Chat] create confirm: delegate returned None (create failed)");
                                        }
                                    } else {
                                        debug!("[Chat] create confirm: no delegate");
                                    }
                                    this.chat_creating = false;
                                    this.chat_create_title = None;
                                    this.chat_create_prompt = None;
                                    this.chat_session_view = None;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .when_some(self.chat_create_title.as_ref(), |this, e| {
                this.child(gpui_component::input::Input::new(e).w_full())
            })
            .child(
                div().w_full().flex_grow().h_0().when_some(
                    self.chat_create_prompt.as_ref(),
                    |this, e| {
                        this.child(
                            gpui_component::input::Input::new(e).w_full().h_full(),
                        )
                    },
                ),
            )
    }

    fn render_chat_session_list(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let muted = theme.muted_foreground;

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .w_full()
                    .px_3()
                    .py_2()
                    .items_center()
                    .justify_between()
                    .child(
                        Label::new(i18n::t(I18nKey::Chat, self.language))
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(muted),
                    )
                    .child(
                        Button::new("new-chat")
                            .ghost()
                            .icon(PdfIconName::ZoomIn)
                            .compact()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chat_creating = true;
                                this.chat_create_title = None;
                                this.chat_create_prompt = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_grow()
                    .h_0()
                    .overflow_y_scrollbar()
                    .px_2()
                    .py_1()
                    .gap_2()
                    .children({
                        let mut cards: Vec<gpui::AnyElement> = Vec::new();
                        let sessions = self.chat_sessions.clone();
                        debug!(
                            "[Chat] render_chat_session_list: {} sessions",
                            sessions.len()
                        );
                        for (i, session) in sessions.iter().enumerate() {
                            let sid = session.id.clone();
                            let title = session.title.clone();
                            let local_time =
                                chrono::DateTime::from_timestamp(session.updated_at, 0)
                                    .map(|dt| dt.with_timezone(&chrono::Local))
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                    .unwrap_or_default();
                            let card = v_flex()
                                .w_full()
                                .group("chat-session-card")
                                .bg(theme.muted.opacity(0.3))
                                .border_1()
                                .border_color(theme.border)
                                .rounded_md()
                                .overflow_hidden()
                                .hover(|s| s.border_color(theme.accent))
                                .cursor_pointer()
                                .on_mouse_down(
                                    gpui::MouseButton::Left,
                                    cx.listener({
                                        let sid = sid.clone();
                                        move |this, _, _, cx| {
                                            debug!("[Chat] select session: sid={sid}");
                                            this.active_chat_session_id = Some(sid.clone());
                                            this.chat_session_view = None;
                                            cx.notify();
                                        }
                                    }),
                                )
                                .child(
                                    h_flex()
                                        .w_full()
                                        .px_2()
                                        .py_1p5()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            div().flex_1().min_w_0().child(
                                                Label::new(title)
                                                    .text_xs()
                                                    .whitespace_nowrap()
                                                    .overflow_hidden()
                                                    .text_ellipsis(),
                                            ),
                                        )
                                        .child(
                                            h_flex()
                                                .gap_0()
                                                .opacity(0.0)
                                                .group_hover("chat-session-card", |s| {
                                                    s.opacity(1.0)
                                                })
                                                .child(
                                                    Button::new(gpui::SharedString::from(format!(
                                                        "chat-delete-{i}"
                                                    )))
                                                    .ghost()
                                                    .icon(PdfIconName::Close)
                                                    .compact()
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        if let Some(delegate) = &this.delegate {
                                                            delegate.delete_chat_session(&sid);
                                                        }
                                                        this.chat_sessions.retain(|s| s.id != sid);
                                                        cx.notify();
                                                    })),
                                                ),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .px_2()
                                        .py_0p5()
                                        .justify_end()
                                        .child(Label::new(local_time).text_xs().text_color(muted)),
                                );
                            cards.push(card.into_any_element());
                        }
                        if self.chat_sessions.is_empty() {
                            cards.push(
                                v_flex()
                                    .size_full()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Label::new(i18n::t(I18nKey::NoChatSessions, self.language))
                                            .text_xs()
                                            .text_color(muted),
                                    )
                                    .into_any_element(),
                            );
                        }
                        cards
                    }),
            )
    }
}
