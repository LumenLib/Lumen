use crate::services::MainApp;
use crate::ui::components::muted_input;
use crate::ui::theme_manager::surface;
use components::IconName;
use gpui::prelude::*;
use gpui::{AppContext, Entity, FocusHandle, MouseButton, Window, actions, div, rems};
use gpui_component::input::InputEvent;
use gpui_component::{
    ActiveTheme, Icon, h_flex,
    input::InputState,
    scroll::ScrollableElement,
    v_flex,
};
use i18n::{I18nKey, t, tf};
use log::debug;
use models::Tag;
use std::sync::Arc;

actions!(tag_selector, [Confirm, Cancel, SelectUp, SelectDown]);

pub type TagSelectCallback =
    Box<dyn Fn(String, &mut Window, &mut Context<TagSelector>) + Send + Sync>;
pub type TagCloseCallback = Box<dyn Fn(&mut Window, &mut Context<TagSelector>) + Send + Sync>;

pub struct TagSelector {
    app: Arc<MainApp>,
    search_input: Entity<InputState>,
    filtered_tags: Vec<Tag>,
    all_tags: Vec<Tag>,
    current_tags: Vec<String>,
    on_select: TagSelectCallback,
    on_close: Option<TagCloseCallback>,
    selected_index: usize,
    query: String,
    focus_handle: FocusHandle,
}

impl TagSelector {
    pub fn build<V: 'static>(
        app: Arc<MainApp>,
        current_tags: Vec<String>,
        window: &mut Window,
        cx: &mut Context<V>,
        on_select: impl Fn(String, &mut Window, &mut Context<Self>) + Send + Sync + 'static,
        on_close: impl Fn(&mut Window, &mut Context<Self>) + Send + Sync + 'static,
    ) -> Entity<Self> {
        let lang = app.current_language();
        let mut all_tags = app
            .db
            .get_all_tags_with_counts()
            .map(|tags| tags.into_iter().map(|(t, _)| t).collect::<Vec<_>>())
            .unwrap_or_default();
        all_tags.sort_by_key(|a| a.name.to_lowercase());

        // Create input entity first using the outer context and window
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(I18nKey::SearchOrCreateTags, lang))
        });

        cx.new(|cx: &mut Context<Self>| {
            // Subscribe to input changes
            cx.subscribe(&search_input, Self::on_input_event).detach();

            let mut this = Self {
                app,
                search_input,
                filtered_tags: all_tags.clone(),
                all_tags,
                current_tags,
                on_select: Box::new(on_select),
                on_close: Some(Box::new(on_close)),
                selected_index: 0,
                query: String::new(),
                focus_handle: cx.focus_handle(),
            };

            this.filter_tags("", cx);
            this
        })
    }

    pub fn on_close(
        mut self,
        callback: impl Fn(&mut Window, &mut Context<Self>) + Send + Sync + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(callback));
        self
    }

    fn filter_tags(&mut self, query: &str, cx: &mut Context<Self>) {
        self.query = query.to_string();

        if query.is_empty() {
            self.filtered_tags = self.all_tags.clone();
        } else {
            let query_lower = query.to_lowercase();
            self.filtered_tags = self
                .all_tags
                .iter()
                .filter(|t| t.name.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
        }
        self.selected_index = 0;
        cx.notify();
    }

    fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.filtered_tags.is_empty() {
            if !self.query.is_empty() {
                debug!("标签选择器: 创建新标签 \"{}\"", self.query);
                (self.on_select)(self.query.clone(), window, cx);
                if let Some(on_close) = &self.on_close {
                    on_close(window, cx);
                }
            }
        } else if self.selected_index < self.filtered_tags.len() {
            let tag = &self.filtered_tags[self.selected_index];
            debug!("标签选择器: 已选择标签 \"{}\"", tag.name);
            (self.on_select)(tag.name.clone(), window, cx);
            if let Some(on_close) = &self.on_close {
                on_close(window, cx);
            }
        } else if !self.query.is_empty() {
            debug!("标签选择器: 创建新标签 \"{}\" (底部选项)", self.query);
            (self.on_select)(self.query.clone(), window, cx);
            if let Some(on_close) = &self.on_close {
                on_close(window, cx);
            }
        }
    }

    fn on_input_event(
        &mut self,
        entity: Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::Change = event {
            let text = entity.read(cx).text();
            self.filter_tags(&text.to_string(), cx);
        }
    }
}

impl Render for TagSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        // Check if query matches any existing tag exactly
        let exact_match = self.filtered_tags.iter().any(|t| t.name == self.query);
        let show_create = !self.query.is_empty() && !exact_match;

        div()
            .track_focus(&self.focus_handle)
            .key_context("TagSelector")
            .on_action(cx.listener(|this, _: &SelectUp, _window, cx| {
                if this.selected_index > 0 {
                    this.selected_index -= 1;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(move |this, _: &SelectDown, _window, cx| {
                let max_index = if show_create {
                    this.filtered_tags.len()
                } else {
                    this.filtered_tags.len().saturating_sub(1)
                };
                if this.selected_index < max_index {
                    this.selected_index += 1;
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                this.confirm_selection(window, cx);
            }))
            // Cancel action to close
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if let Some(on_close) = &this.on_close {
                    on_close(window, cx);
                }
            }))
            .child(
                v_flex()
                    .p_2()
                    .gap_2()
                    .child(
                        // Pass reference to entity
                        muted_input(&self.search_input, &theme),
                    )
                    .child(
                        v_flex()
                            .max_h(rems(12.5))
                            .overflow_y_scrollbar()
                            .gap_1()
                            .children(self.filtered_tags.iter().enumerate().map(|(ix, tag)| {
                                let is_selected = ix == self.selected_index;
                                let is_active = self.current_tags.contains(&tag.name);
                                let color_str = tag.color.clone();
                                let tag_name = tag.name.clone();

                                // Parse color safely
                                let bg_color =
                                    if let Ok(rgba) = gpui::Rgba::try_from(color_str.as_str()) {
                                        gpui::Hsla::from(rgba)
                                    } else {
                                        theme.primary
                                    };

                                h_flex()
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .items_center()
                                    .justify_between()
                                    .bg(if is_selected {
                                        surface().hover_bg
                                    } else {
                                        gpui::Hsla::default()
                                    })
                                    .hover(|s| s.bg(theme.muted))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            (this.on_select)(tag_name.clone(), window, cx);
                                            if let Some(on_close) = &this.on_close {
                                                on_close(window, cx);
                                            }
                                        }),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .w(rems(0.5))
                                                    .h(rems(0.5))
                                                    .rounded_full()
                                                    .bg(bg_color),
                                            )
                                            .child(div().text_sm().child(tag.name.clone())),
                                    )
                                    .child(if is_active {
                                        Icon::new(IconName::Check)
                                            .size(rems(0.875))
                                            .text_color(theme.primary)
                                            .into_any_element()
                                    } else {
                                        div().into_any_element()
                                    })
                            }))
                            .child(if show_create {
                                let is_selected = self.selected_index == self.filtered_tags.len();
                                h_flex()
                                    .w_full()
                                    .px_2()
                                    .py_1()
                                    .rounded_sm()
                                    .items_center()
                                    .gap_2()
                                    .bg(if is_selected {
                                        surface().hover_bg
                                    } else {
                                        gpui::Hsla::default()
                                    })
                                    .hover(|s| s.bg(theme.muted))
                                    .cursor_pointer()
                                    .text_color(theme.primary)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            (this.on_select)(this.query.clone(), window, cx);
                                            if let Some(on_close) = &this.on_close {
                                                on_close(window, cx);
                                            }
                                        }),
                                    )
                                    .child(Icon::new(IconName::Plus).size(rems(0.875)))
                                    .child(div().text_sm().child(tf(
                                        I18nKey::CreateTag,
                                        lang,
                                        &[&self.query],
                                    )))
                                    .into_any_element()
                            } else {
                                h_flex().into_any_element()
                            }),
                    ),
            )
    }
}
