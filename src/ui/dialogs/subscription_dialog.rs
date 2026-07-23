use components::IconName;
use gpui::prelude::*;
use gpui::{AppContext, Entity, SharedString, Window, div};
use gpui_component::{
    ActiveTheme, Icon, h_flex,
    input::{Input, InputState},
    v_flex,
};
use i18n::{I18nKey, Language, t};
use models::Feed;
use services::app::MainApp;
use std::sync::Arc;

pub struct SubscriptionDialog {
    pub(crate) app: Arc<MainApp>,
    pub(crate) name_input: Entity<InputState>,
    pub(crate) url_input: Entity<InputState>,
    pub(crate) interval_input: Entity<InputState>,
    pub(crate) is_edit: bool,
    pub(crate) feed_id: Option<String>,
}

impl SubscriptionDialog {
    fn labeled_input(
        &self,
        label: impl Into<SharedString>,
        input: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let theme = cx.theme();
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(label.into()),
            )
            .child(
                div()
                    .bg(theme.background)
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(Input::new(input).appearance(false)),
            )
    }

    pub fn new(
        app: Arc<MainApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
        existing_feed: Option<Feed>,
    ) -> Self {
        let lang = app.current_language();
        let is_edit = existing_feed.is_some();
        let feed_id = existing_feed.as_ref().map(|f| f.id.clone());
        let (name, url, interval) = if let Some(f) = existing_feed {
            (
                f.name,
                f.url.unwrap_or_default(),
                f.update_interval.to_string(),
            )
        } else {
            (String::new(), String::new(), "24".to_string())
        };

        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(name)
                .placeholder(t(I18nKey::SubscriptionNamePlaceholder, lang))
        });
        let url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(url)
                .placeholder(t(I18nKey::SubscriptionUrlPlaceholder, lang))
        });
        let interval_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(interval)
                .placeholder(t(I18nKey::UpdateIntervalPlaceholder, lang))
        });

        Self {
            app,
            name_input,
            url_input,
            interval_input,
            is_edit,
            feed_id,
        }
    }

    /// 更新间隔行：输入框占满剩余宽度，编辑态在右侧追加一个"立即更新"图标按钮。
    fn interval_row(&self, lang: Language, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme().clone();
        let muted_foreground = theme.muted_foreground;
        let secondary_hover = theme.secondary_hover;
        h_flex()
            .items_center()
            .gap_2()
            .child(
                self.labeled_input(t(I18nKey::UpdateInterval, lang), &self.interval_input, cx)
                    .flex_1(),
            )
            .when(self.is_edit, |row| {
                let app = self.app.clone();
                let feed_id = self.feed_id.clone();
                row.child(
                    div()
                        .id("subscription-refresh-btn")
                        .cursor_pointer()
                        .p_2()
                        .rounded_md()
                        .text_color(muted_foreground)
                        .hover(|s| s.bg(secondary_hover))
                        .child(Icon::new(IconName::RotateCw))
                        .on_click(move |_, _window, _cx| {
                            if let Some(id) = feed_id.as_deref() {
                                let _ = app.refresh_feed(id);
                            }
                        }),
                )
            })
    }
}

impl Render for SubscriptionDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();
        let app = self.app.clone();
        let feed_id = self.feed_id.clone();
        let is_edit = self.is_edit;

        div()
            .size_full()
            .bg(theme.muted)
            .p_4()
            .on_key_down(move |event, _window, _cx| {
                // Enter 触发与右侧图标按钮相同的"立即更新"逻辑（仅编辑态）
                if is_edit
                    && event.keystroke.key == "enter"
                    && let Some(id) = feed_id.as_deref()
                {
                    let _ = app.refresh_feed(id);
                }
            })
            .child(
                v_flex()
                    .gap_3()
                    .child(self.labeled_input(t(I18nKey::FeedName, lang), &self.name_input, cx))
                    .child(self.labeled_input(t(I18nKey::FeedUrl, lang), &self.url_input, cx))
                    .child(self.interval_row(lang, cx)),
            )
    }
}
