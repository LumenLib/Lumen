use super::LabeledInput;
use crate::services::MainApp;
use crate::ui::icons::IconName;
use crate::ui::theme_manager::surface;
use gpui::prelude::*;
use gpui::{AppContext, Entity, FontWeight, Window, WindowControlArea, div, rems};
use gpui_component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    h_flex,
    input::InputState,
    v_flex,
};
use i18n::{I18nKey, t};
use log::debug;
use models::Feed;
use std::sync::Arc;

pub type SubscriptionConfirmCallback =
    Box<dyn Fn(String, String, u32, &mut Window, &mut Context<SubscriptionEditor>) + Send + Sync>;
pub struct SubscriptionEditor {
    app: Arc<MainApp>,
    name_input: Entity<InputState>,
    url_input: Entity<InputState>,
    interval_input: Entity<InputState>,
    is_edit: bool,
    on_confirm: SubscriptionConfirmCallback,
}

impl SubscriptionEditor {
    pub fn new(
        app: Arc<MainApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
        existing_feed: Option<Feed>,
        on_confirm: impl Fn(String, String, u32, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let lang = app.current_language();
        let is_edit = existing_feed.is_some();
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
            on_confirm: Box::new(on_confirm),
        }
    }
}

impl Render for SubscriptionEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        let title = if self.is_edit {
            t(I18nKey::EditSubscription, lang)
        } else {
            t(I18nKey::AddSubscription, lang)
        };
        let confirm_text = if self.is_edit {
            t(I18nKey::Save, lang)
        } else {
            t(I18nKey::Add, lang)
        };

        div()
            .size_full()
            .px_6()
            .pt(rems(2.0))
            .pb_6()
            .bg(theme.background)
            .when(cfg!(not(target_os = "macos")), |this: gpui::Div| {
                this.child(
                    div()
                        .h(rems(2.0))
                        .w_full()
                        .absolute()
                        .top_0()
                        .left_0()
                        .window_control_area(WindowControlArea::Drag),
                )
                // Window controls (Close only for modal)
                .child(
                    div()
                        .absolute()
                        .top_1()
                        .right_1()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id("modal-close-btn")
                                .h(rems(1.5))
                                .w(rems(1.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .cursor_pointer()
                                .occlude()
                                .window_control_area(WindowControlArea::Close)
                                .hover(|s| s.bg(surface().danger_hover))
                                .child(
                                    Icon::new(IconName::Close)
                                        .size(rems(0.875))
                                        .text_color(theme.foreground),
                                ),
                        ),
                )
            })
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .mb_2()
                            .child(div().text_lg().font_weight(FontWeight::BOLD).child(title))
                            .child(
                                h_flex().gap_2().child(
                                    Button::new("sub-confirm")
                                        .child(confirm_text)
                                        .primary()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            let name = this.name_input.read(cx).text().to_string();
                                            let url = this.url_input.read(cx).text().to_string();
                                            let interval = this
                                                .interval_input
                                                .read(cx)
                                                .text()
                                                .to_string()
                                                .parse::<u32>()
                                                .unwrap_or(24);

                                            if !name.is_empty() && !url.is_empty() {
                                                debug!("订阅确认: name={name}, url={url}, interval={interval}");
                                                (this.on_confirm)(name, url, interval, window, cx);
                                            }
                                        })),
                                ),
                            ),
                    )
                    .child(LabeledInput::new(
                        t(I18nKey::FeedName, lang),
                        &self.name_input,
                    ))
                    .child(LabeledInput::new(
                        t(I18nKey::FeedUrl, lang),
                        &self.url_input,
                    ))
                    .child(LabeledInput::new(
                        t(I18nKey::UpdateInterval, lang),
                        &self.interval_input,
                    )),
            )
    }
}
