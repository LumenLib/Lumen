use services::app::MainApp;
use gpui::prelude::*;
use gpui::{AppContext, Entity, SharedString, Window, div};
use gpui_component::{ActiveTheme, input::{Input, InputState}, v_flex};
use i18n::{I18nKey, t};
use models::Feed;
use std::sync::Arc;

pub struct SubscriptionDialogContent {
    pub(crate) app: Arc<MainApp>,
    pub(crate) name_input: Entity<InputState>,
    pub(crate) url_input: Entity<InputState>,
    pub(crate) interval_input: Entity<InputState>,
    pub(crate) is_edit: bool,
    pub(crate) feed_id: Option<String>,
}

impl SubscriptionDialogContent {
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
}

impl Render for SubscriptionDialogContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        div()
            .size_full()
            .bg(theme.muted)
            .p_4()
            .child(
                v_flex()
                    .gap_3()
                    .child(self.labeled_input(
                        t(I18nKey::FeedName, lang),
                        &self.name_input,
                        cx,
                    ))
                    .child(self.labeled_input(
                        t(I18nKey::FeedUrl, lang),
                        &self.url_input,
                        cx,
                    ))
                    .child(self.labeled_input(
                        t(I18nKey::UpdateInterval, lang),
                        &self.interval_input,
                        cx,
                    )),
            )
    }
}
