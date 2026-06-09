use crate::view::PdfReaderView;
use crate::view::components::edit_chat_dialog::EditChatSessionDialog;
use crate::view::types::PdfIconName;
use crate::PdfReaderDelegate;
use gpui::prelude::*;
use gpui::{
    Bounds, Context, Point, TitlebarOptions, WeakEntity, Window,
    WindowBounds, WindowKind, WindowOptions, div, list, px, relative, size,
};
use gpui_component::Root;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::text::TextView;
use gpui_component::{ActiveTheme, Disableable, Icon, h_flex, label::Label, v_flex};
use i18n::{I18nKey, Language};
use log::debug;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) struct ChatSessionView {
    delegate: Option<Arc<dyn PdfReaderDelegate>>,
    language: Language,
    session_id: String,
    session_title: String,
    system_prompt: String,

    chat_messages: Vec<models::chat::ChatMessage>,
    chat_streaming_message: String,
    is_chat_streaming: bool,
    chat_input_state: Option<gpui::Entity<InputState>>,
    chat_input_sub: Option<gpui::Subscription>,
    list_state: gpui::ListState,
    chat_quote_expanded: std::collections::HashSet<i64>,
    chat_selected_attachments: Vec<models::Attachment>,
    chat_show_attachment_picker: bool,

    parent_handle: WeakEntity<PdfReaderView>,
}

impl ChatSessionView {
    pub(crate) fn new(
        delegate: Option<Arc<dyn PdfReaderDelegate>>,
        language: Language,
        session_id: String,
        session_title: String,
        system_prompt: String,
        messages: Vec<models::chat::ChatMessage>,
        parent: WeakEntity<PdfReaderView>,
        _cx: &mut Context<Self>,
    ) -> Self {
        let msg_count = messages.len();
        Self {
            delegate,
            language,
            session_id,
            session_title,
            system_prompt,
            chat_messages: messages,
            chat_streaming_message: String::new(),
            is_chat_streaming: false,
            chat_input_state: None,
            chat_input_sub: None,
            list_state: gpui::ListState::new(
                msg_count,
                gpui::ListAlignment::Bottom,
                px(1000.),
            ),
            chat_quote_expanded: std::collections::HashSet::new(),
            chat_selected_attachments: Vec::new(),
            chat_show_attachment_picker: false,
            parent_handle: parent,
        }
    }

    fn send_chat_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self
            .chat_input_state
            .as_ref()
            .map(|e| e.read(cx).text().to_string())
            .unwrap_or_default();
        if input.is_empty() {
            return;
        }

        let session_id = self.session_id.clone();
        debug!(
            "[Chat] send_chat_message: session_id={session_id}, input_len={}",
            input.len()
        );

        if let Some(e) = &self.chat_input_state {
            e.update(cx, |s, cx| {
                s.set_value("", window, cx);
            });
        }

        let attachment_paths: Vec<String> = self
            .chat_selected_attachments
            .iter()
            .map(|a| a.file_path.clone())
            .collect();

        if let Some(ref delegate) = self.delegate {
            delegate.add_chat_message(&session_id, "user", &input, &attachment_paths);
        }

        let now = chrono::Utc::now().timestamp();
        let user_msg = models::chat::ChatMessage {
            id: String::new(),
            session_id: session_id.clone(),
            role: "user".to_string(),
            content: input.clone(),
            attachments: attachment_paths,
            created_at: now,
        };
        self.chat_messages.push(user_msg);
        self.chat_selected_attachments.clear();

        self.start_chat_stream(session_id, cx);
    }

    fn start_chat_stream(&mut self, session_id: String, cx: &mut Context<Self>) {
        let messages_for_ai: Vec<models::chat::ChatMessage> = self.chat_messages.clone();
        let system_prompt = self.system_prompt.clone();

        debug!(
            "[Chat] start_chat_stream: session_id={session_id}, messages_count={}, system_prompt_len={}",
            messages_for_ai.len(),
            system_prompt.len(),
        );

        self.is_chat_streaming = true;
        self.chat_streaming_message = String::new();
        self.list_state
            .reset(self.chat_messages.len() + (self.is_chat_streaming as usize));
        cx.notify();

        if let Some(ref delegate) = self.delegate.clone() {
            let delegate = delegate.clone();
            cx.spawn(|this: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    match delegate
                        .chat_stream(
                            session_id.clone(),
                            messages_for_ai.clone(),
                            system_prompt.clone(),
                        )
                        .await
                    {
                        Ok(mut rx) => {
                            let notify_interval = Duration::from_millis(50);
                            let mut last_notify = Instant::now() - notify_interval;
                            while let Some(token) = rx.recv().await {
                                let now = Instant::now();
                                let should_notify =
                                    now.duration_since(last_notify) >= notify_interval;
                                let _ = this.update(&mut cx, |this, cx| {
                                    this.chat_streaming_message.push_str(&token);
                                    if should_notify {
                                        cx.notify();
                                    }
                                });
                                if should_notify {
                                    last_notify = now;
                                }
                            }
                            // Always do one final notify after stream ends
                            let _ = this.update(&mut cx, |this, cx| {
                                let msg = std::mem::take(&mut this.chat_streaming_message);
                                this.is_chat_streaming = false;
                                let sid = session_id.clone();
                                if let Some(ref delegate) = this.delegate {
                                    delegate.add_chat_message(&sid, "assistant", &msg, &[]);
                                }
                                let assistant_msg = models::chat::ChatMessage {
                                    id: String::new(),
                                    session_id: sid,
                                    role: "assistant".to_string(),
                                    content: msg,
                                    attachments: Vec::new(),
                                    created_at: chrono::Utc::now().timestamp(),
                                };
                                this.chat_messages.push(assistant_msg);
                                this.list_state.reset(
                                    this.chat_messages.len()
                                        + (this.is_chat_streaming as usize),
                                );
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            let _ = this.update(&mut cx, |this, cx| {
                                this.is_chat_streaming = false;
                                let err_msg = format!("Error: {e}");
                                let sid = session_id.clone();
                                if let Some(ref delegate) = this.delegate {
                                    delegate.add_chat_message(&sid, "assistant", &err_msg, &[]);
                                }
                                this.chat_messages.push(models::chat::ChatMessage {
                                    id: String::new(),
                                    session_id: sid,
                                    role: "assistant".to_string(),
                                    content: err_msg,
                                    attachments: Vec::new(),
                                    created_at: chrono::Utc::now().timestamp(),
                                });
                                this.list_state.reset(
                                    this.chat_messages.len()
                                        + (this.is_chat_streaming as usize),
                                );
                                cx.notify();
                            });
                        }
                    }
                }
            })
            .detach();
        }
    }

    fn render_chat_bubble(
        &self,
        msg: &models::chat::ChatMessage,
        theme: &gpui_component::Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_user = msg.role == "user";
        let is_quote = msg.role == "quote";
        let is_streaming = msg.role == "assistant" && msg.id.is_empty() && msg.created_at == 0;

        let bubble_color = if is_user { theme.primary } else { theme.muted };

        let display_content = if is_streaming && msg.content.is_empty() {
            i18n::t(I18nKey::AiThinking, self.language).to_string()
        } else {
            msg.content.clone()
        };

        let cursor = if is_streaming {
            format!("{} ▊", &display_content)
        } else {
            display_content.clone()
        };

        if is_quote {
            let expanded = self.chat_quote_expanded.contains(&msg.created_at);
            let is_long = display_content.len() > 100;
            let created_at = msg.created_at;
            return v_flex().w_full().items_end().child(
                v_flex()
                    .w(relative(0.8))
                    .bg(theme.muted.opacity(0.06))
                    .border_l_1()
                    .border_color(theme.accent)
                    .rounded_md()
                    .px_2()
                    .py_1p5()
                    .gap_1()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .child(
                                Label::new(i18n::t(I18nKey::QuoteLabel, self.language))
                                    .text_xs()
                                    .text_color(theme.accent),
                            )
                            .when(is_long, |this| {
                                this.child(
                                    div()
                                        .id(gpui::SharedString::from(format!(
                                            "quote-toggle-{}",
                                            created_at
                                        )))
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                if this.chat_quote_expanded.contains(&created_at) {
                                                    this.chat_quote_expanded.remove(&created_at);
                                                } else {
                                                    this.chat_quote_expanded.insert(created_at);
                                                }
                                                cx.notify();
                                            }),
                                        )
                                        .child(
                                            Icon::new(if expanded {
                                                PdfIconName::ChevronDown
                                            } else {
                                                PdfIconName::ChevronRight
                                            })
                                            .size(px(12.0)),
                                        ),
                                )
                            }),
                    )
                    .child(if expanded || !is_long {
                        Label::new(display_content)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .into_any_element()
                    } else {
                        Label::new(display_content)
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .into_any_element()
                    }),
            );
        }

        let has_attachments = !msg.attachments.is_empty() && !is_quote;
        v_flex()
            .w_full()
            .when(is_user, |this| this.items_end())
            .when(!is_user, |this| this.items_start())
            .child(
                v_flex()
                    .when(is_user, |this| this.w(relative(0.8)))
                    .bg(bubble_color.opacity(if is_user { 0.85 } else { 0.15 }))
                    .rounded_md()
                    .px_2()
                    .py_1()
                    .gap_0p5()
                    .when(has_attachments, |this| {
                        this.children(msg.attachments.iter().map(|fp| {
                            let name = std::path::Path::new(fp)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(fp);
                            h_flex()
                                .gap_1()
                                .items_center()
                                .child(Icon::new(PdfIconName::Pin).size(px(12.0)).text_color(
                                    if is_user {
                                        gpui::white().opacity(0.7)
                                    } else {
                                        theme.muted_foreground
                                    },
                                ))
                                .child(Label::new(name.to_string()).text_xs().text_color(
                                    if is_user {
                                        gpui::white().opacity(0.7)
                                    } else {
                                        theme.muted_foreground
                                    },
                                ))
                                .into_any_element()
                        }))
                    })
                    .child(
                        TextView::markdown(
                            gpui::SharedString::from(format!(
                                "chat-msg-{}-{}",
                                msg.role, msg.created_at
                            )),
                            &cursor,
                            window,
                            cx,
                        )
                        .selectable(true)
                        .text_xs()
                        .text_color(if is_user {
                            gpui::white()
                        } else {
                            theme.foreground
                        }),
                    ),
            )
    }
}

impl gpui::Render for ChatSessionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;

        if self.chat_input_state.is_none() {
            let entity = cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder(i18n::t(I18nKey::ChatInputPlaceholder, self.language))
                    .auto_grow(2, 10)
            });
            self.chat_input_state = Some(entity);
        }

        if self.chat_input_state.is_some() && self.chat_input_sub.is_none() {
            if let Some(entity) = &self.chat_input_state {
                let sub = cx.subscribe(entity, |this, _, event: &InputEvent, cx| {
                    if let InputEvent::PressEnter { secondary } = event {
                        if *secondary {
                            return;
                        }
                        // Enter without Shift → send
                        let text = this
                            .chat_input_state
                            .as_ref()
                            .map(|e| e.read(cx).text().to_string())
                            .unwrap_or_default();
                        // Strip trailing \n (multi-line mode inserts \n before PressEnter)
                        let trimmed = text.trim_end_matches('\n').to_string();
                        if trimmed.is_empty() {
                            return;
                        }
                        let session_id = this.session_id.clone();
                        if let Some(ref delegate) = this.delegate {
                            delegate.add_chat_message(&session_id, "user", &trimmed, &[]);
                        }
                        let now = chrono::Utc::now().timestamp();
                        this.chat_messages.push(models::chat::ChatMessage {
                            id: String::new(),
                            session_id: session_id.clone(),
                            role: "user".to_string(),
                            content: trimmed,
                            attachments: Vec::new(),
                            created_at: now,
                        });
                        this.chat_input_state = None;
                        this.chat_input_sub = None;
                        this.start_chat_stream(session_id, cx);
                        cx.notify();
                    }
                });
                self.chat_input_sub = Some(sub);
            }
        }

        v_flex()
            .size_full()
            .child(
                // ── 顶部栏 ──
                h_flex()
                    .w_full()
                    .px_2()
                    .py_1()
                    .border_b_1()
                    .border_color(theme.border)
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("chat-back")
                            .ghost()
                            .icon(PdfIconName::ChevronLeft)
                            .compact()
                            .on_click({
                                let parent = self.parent_handle.clone();
                                cx.listener(move |_this, _, _, cx| {
                                    if let Some(parent) = parent.upgrade() {
                                        parent.update(cx, |parent, cx| {
                                            parent.active_chat_session_id = None;
                                            parent.chat_session_view = None;
                                            cx.notify();
                                        });
                                    }
                                })
                            }),
                    )
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(self.session_title.clone())
                                .text_xs()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis(),
                        ),
                    )
                    .child(
                        Button::new("chat-edit-prompt")
                            .ghost()
                            .icon(PdfIconName::Annotations)
                            .compact()
                            .tooltip(i18n::t(I18nKey::EditSystemPrompt, self.language))
                            .on_click({
                                let parent = self.parent_handle.clone();
                                let sid = self.session_id.clone();
                                let title = self.session_title.clone();
                                let prompt = self.system_prompt.clone();
                                let lang = self.language;
                                cx.listener(move |this, _, _window, cx| {
                                    let delegate = this.delegate.clone();
                                    let parent = parent.clone();
                                    let title = title.clone();
                                    let prompt = prompt.clone();
                                    let sid = sid.clone();
                                    let size_val = size(px(480.0), px(400.0));
                                    let bounds = Bounds::centered(None, size_val, cx);
                                    let _ = cx.open_window(
                                        WindowOptions {
                                            window_bounds: Some(WindowBounds::Windowed(bounds)),
                                            titlebar: Some(TitlebarOptions {
                                                title: None,
                                                appears_transparent: true,
                                                traffic_light_position: Some(Point::new(
                                                    px(9.0),
                                                    px(9.0),
                                                )),
                                            }),
                                            is_resizable: false,
                                            is_minimizable: false,
                                            kind: WindowKind::Floating,
                                            ..Default::default()
                                        },
                                        move |window, cx| {
                                            let dialog = cx.new(|cx| {
                                                EditChatSessionDialog::new(
                                                    title.clone(),
                                                    prompt.clone(),
                                                    lang,
                                                    window,
                                                    cx,
                                                    move |result, _window, cx| {
                                                        if let Some((new_title, new_prompt)) =
                                                            result
                                                        {
                                                            if let Some(ref delegate) = delegate {
                                                                delegate.update_chat_session(
                                                                    &sid,
                                                                    Some(&new_title),
                                                                    Some(&new_prompt),
                                                                );
                                                            }
                                                            if let Some(parent) = parent.upgrade()
                                                            {
                                                                parent.update(cx, |parent, cx| {
                                                                    if let Some(s) = parent
                                                                        .chat_sessions
                                                                        .iter_mut()
                                                                        .find(|s| s.id == sid)
                                                                    {
                                                                        s.title = new_title.clone();
                                                                        s.system_prompt =
                                                                            new_prompt.clone();
                                                                    }
                                                                    if let Some(ref entity) =
                                                                        parent.chat_session_view
                                                                    {
                                                                        entity.update(
                                                                            cx,
                                                                            |chat, cx| {
                                                                                chat.session_title =
                                                                                    new_title;
                                                                                chat.system_prompt =
                                                                                    new_prompt;
                                                                                cx.notify();
                                                                            },
                                                                        );
                                                                    }
                                                                    _window.remove_window();
                                                                });
                                                            }
                                                        } else {
                                                            _window.remove_window();
                                                        }
                                                    },
                                                )
                                            });
                                            let root =
                                                cx.new(|cx| Root::new(dialog, window, cx));
                                            root
                                        },
                                    );
                                })
                            }),
                    ),
            )
            .child(
                // ── 消息区域 ──
                v_flex()
                    .flex_grow()
                    .h_0()
                    .relative()
                    .px_2()
                    .py_2()
                    .child(
                        if self.chat_messages.is_empty() && !self.is_chat_streaming {
                            v_flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .child(
                                    Label::new(i18n::t(
                                        I18nKey::ChatInputPlaceholder,
                                        self.language,
                                    ))
                                    .text_xs()
                                    .text_color(muted),
                                )
                                .into_any_element()
                        } else {
                            let view = cx.entity().downgrade();
                            let theme = theme.clone();
                            list(
                                self.list_state.clone(),
                                move |ix, window, cx| {
                                    let result =
                                        view.update(cx, |this, cx| {
                                            if ix < this.chat_messages.len() {
                                                this.render_chat_bubble(
                                                    &this.chat_messages[ix],
                                                    &theme,
                                                    window,
                                                    cx,
                                                )
                                                .into_any_element()
                                            } else if this.is_chat_streaming {
                                                let streaming_msg =
                                                    models::chat::ChatMessage {
                                                        id: String::new(),
                                                        session_id: String::new(),
                                                        role: "assistant"
                                                            .to_string(),
                                                        content: this
                                                            .chat_streaming_message
                                                            .clone(),
                                                        attachments: Vec::new(),
                                                        created_at: 0,
                                                    };
                                                this.render_chat_bubble(
                                                    &streaming_msg,
                                                    &theme,
                                                    window,
                                                    cx,
                                                )
                                                .into_any_element()
                                            } else {
                                                div().into_any_element()
                                            }
                                        });
                                    result
                                        .unwrap_or_else(|_| div().into_any_element())
                                },
                            )
                            .size_full()
                            .into_any_element()
                        },
                    ),
            )
            .child(
                // ── 输入栏 ──
                {
                    let mut input_section = v_flex()
                        .w_full()
                        .px_2()
                        .py_1p5()
                        .border_t_1()
                        .border_color(theme.border);

                    if self.chat_show_attachment_picker {
                        let attachments = self
                            .delegate
                            .as_ref()
                            .map(|d| d.current_literature_attachments())
                            .unwrap_or_default();
                        let selected_ids: Vec<String> = self
                            .chat_selected_attachments
                            .iter()
                            .map(|a| a.id.clone())
                            .collect();
                        input_section = input_section.child(
                            v_flex()
                                .w_full()
                                .mb_1()
                                .p_1()
                                .bg(theme.muted.opacity(0.06))
                                .rounded_md()
                                .gap_0p5()
                                .border_1()
                                .border_color(theme.border)
                                .children(attachments.iter().map(|att| {
                                    let is_selected = selected_ids.contains(&att.id);
                                    let file_path = att.file_path.clone();
                                    let file_name = att.file_name.clone();
                                    let att_id = att.id.clone();
                                    h_flex()
                                        .w_full()
                                        .gap_1()
                                        .items_center()
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            gpui::MouseButton::Left,
                                            cx.listener(move |this, _, _, _cx| {
                                                if is_selected {
                                                    this.chat_selected_attachments
                                                        .retain(|a| a.id != att_id);
                                                } else {
                                                    this.chat_selected_attachments.push(
                                                        models::Attachment {
                                                            id: att_id.clone(),
                                                            literature_id: String::new(),
                                                            file_path: file_path.clone(),
                                                            file_name: file_name.clone(),
                                                            file_size: 0,
                                                            mime_type: None,
                                                            etag: None,
                                                            is_main: false,
                                                            is_dirty: false,
                                                            is_deleted: false,
                                                            version: 0,
                                                            created_at: String::new(),
                                                            updated_at: String::new(),
                                                        },
                                                    );
                                                }
                                                _cx.notify();
                                            }),
                                        )
                                        .child(
                                            Icon::new(if is_selected {
                                                PdfIconName::Check
                                            } else {
                                                PdfIconName::Square
                                            })
                                            .size(px(14.0))
                                            .text_color(if is_selected {
                                                theme.primary
                                            } else {
                                                muted
                                            }),
                                        )
                                        .child(
                                            Label::new(att.file_name.clone())
                                                .text_xs()
                                                .text_color(theme.foreground),
                                        )
                                        .into_any_element()
                                })),
                        );
                    }

                    if !self.chat_selected_attachments.is_empty() {
                        let chips = self.chat_selected_attachments.clone();
                        input_section = input_section.child(
                            h_flex().w_full().mb_1().gap_1().flex_wrap().children(
                                chips.iter().map(|att| {
                                    let att_id = att.id.clone();
                                    h_flex()
                                        .bg(theme.muted.opacity(0.1))
                                        .rounded_sm()
                                        .px_1()
                                        .py_0p5()
                                        .gap_0p5()
                                        .items_center()
                                        .child(
                                            Icon::new(PdfIconName::Pin)
                                                .size(px(10.0))
                                                .text_color(muted),
                                        )
                                        .child(
                                            Label::new(att.file_name.clone())
                                                .text_xs()
                                                .text_color(theme.foreground),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .on_mouse_down(
                                                    gpui::MouseButton::Left,
                                                    cx.listener(move |this, _, _, _cx| {
                                                        this.chat_selected_attachments
                                                            .retain(|a| a.id != att_id);
                                                        _cx.notify();
                                                    }),
                                                )
                                                .child(
                                                    Icon::new(PdfIconName::Close)
                                                        .size(px(10.0))
                                                        .text_color(muted),
                                                ),
                                        )
                                        .into_any_element()
                                }),
                            ),
                        );
                    }

                    input_section
                }
                .child({
                    let parent = self.parent_handle.clone();
                    let has_selection = parent
                        .upgrade()
                        .and_then(|p| {
                            p.read(cx)
                                .selected_text
                                .as_ref()
                                .map(|t| !t.is_empty())
                        })
                        .unwrap_or(false);
                    let sid = self.session_id.clone();
                    let lang = self.language;
                    let mut row = h_flex().w_full().gap_1();
                    if has_selection {
                        let parent = parent.clone();
                        row = row.child(
                            Button::new("chat-send-selection")
                                .ghost()
                                .icon(PdfIconName::ZoomIn)
                                .compact()
                                .text_xs()
                                .text_color(theme.primary)
                                .tooltip(i18n::t(I18nKey::SendSelection, lang))
                                .on_click({
                                    let parent = parent.clone();
                                    let sid = sid.clone();
                                    cx.listener(move |this, _, _window, cx| {
                                        if this.is_chat_streaming {
                                            return;
                                        }
                                        let text = parent
                                            .upgrade()
                                            .and_then(|p| {
                                                p.read(cx).selected_text.clone()
                                            })
                                            .unwrap_or_default();
                                        if text.is_empty() {
                                            return;
                                        }
                                        let now = chrono::Utc::now().timestamp();
                                        this.chat_messages.push(
                                            models::chat::ChatMessage {
                                                id: String::new(),
                                                session_id: sid.clone(),
                                                role: "quote".to_string(),
                                                content: text.clone(),
                                                attachments: Vec::new(),
                                                created_at: now,
                                            },
                                        );
                                        this.list_state.reset(
                                            this.chat_messages.len()
                                                + (this.is_chat_streaming as usize),
                                        );
                                        if let Some(ref delegate) = this.delegate {
                                            delegate.add_chat_message(
                                                &sid,
                                                "quote",
                                                &text,
                                                &[],
                                            );
                                        }
                                        cx.notify();
                                    })
                                }),
                        );
                    }
                    row
                })
                .child(
                    h_flex()
                        .w_full()
                        .gap_1()
                        .items_end()
                        .child(div().flex_1().when_some(
                            self.chat_input_state.as_ref(),
                            |this, e| {
                                this.child(
                                    Input::new(e).w_full(),
                                )
                            },
                        ))
                        .child(
                            Button::new("chat-attach")
                                .ghost()
                                .icon(PdfIconName::Pin)
                                .compact()
                                .disabled(self.is_chat_streaming)
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.chat_show_attachment_picker =
                                        !this.chat_show_attachment_picker;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("chat-send")
                                .ghost()
                                .icon(PdfIconName::FastForward)
                                .disabled(self.is_chat_streaming)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.send_chat_message(window, cx);
                                })),
                        ),
                ),
            )
    }
}
