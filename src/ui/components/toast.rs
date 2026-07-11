use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    AnyElement, AsyncApp, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, Window, div, rems,
};
use gpui_component::notification::NotificationType;
use gpui_component::{ActiveTheme, Icon, h_flex, v_flex};

use crate::notification_bus::NotificationBus;
use crate::ui::icons::IconName;

struct ToastItem {
    id: u64,
    ty: NotificationType,
    message: SharedString,
}

pub struct ToastOverlay {
    items: Vec<ToastItem>,
    next_id: u64,
}

impl ToastOverlay {
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            items: Vec::new(),
            next_id: 0,
        }
    }

    fn icon_for(ty: NotificationType) -> IconName {
        match ty {
            NotificationType::Error => IconName::CircleX,
            NotificationType::Warning => IconName::TriangleAlert,
            NotificationType::Info => IconName::Info,
            NotificationType::Success => IconName::Check,
        }
    }

    fn color_for(ty: NotificationType, theme: &gpui_component::Theme) -> gpui::Hsla {
        match ty {
            NotificationType::Error => theme.danger,
            NotificationType::Warning => theme.warning,
            NotificationType::Info => theme.info,
            NotificationType::Success => theme.success,
        }
    }

    fn dismiss_duration(ty: NotificationType) -> Option<Duration> {
        match ty {
            NotificationType::Error => None,
            NotificationType::Warning => Some(Duration::from_secs(5)),
            NotificationType::Info => Some(Duration::from_secs(3)),
            NotificationType::Success => Some(Duration::from_secs(3)),
        }
    }
}

impl Render for ToastOverlay {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bus = cx.global_mut::<NotificationBus>();
        for item in bus.drain() {
            let id = self.next_id;
            self.next_id += 1;
            self.items.push(ToastItem {
                id,
                ty: item.ty,
                message: item.message,
            });

            if let Some(dur) = ToastOverlay::dismiss_duration(item.ty) {
                let this = cx.entity().downgrade();
                cx.spawn(move |_, cx: &mut AsyncApp| {
                    let mut cx = cx.clone();
                    async move {
                        cx.background_executor().timer(dur).await;
                        this.update(&mut cx, |this, cx| {
                            this.items.retain(|i| i.id != id);
                            cx.notify();
                        })
                        .ok();
                    }
                })
                .detach();
            }
        }

        if self.items.is_empty() {
            return div().into_any_element();
        }

        let theme = cx.theme().clone();
        let this = cx.entity().downgrade();

        let rendered: Vec<AnyElement> = {
            let items = &self.items;
            items
                .iter()
                .map(|item| {
                    let color = ToastOverlay::color_for(item.ty, &theme);
                    let icon = ToastOverlay::icon_for(item.ty);
                    let id = item.id;
                    let message = item.message.clone();
                    let close = this.clone();

                    h_flex()
                        .w(rems(20.0))
                        .bg(color)
                        .rounded_lg()
                        .overflow_hidden()
                        .border_1()
                        .border_color(theme.border)
                        .shadow_lg()
                        .child(
                            h_flex()
                                .flex_1()
                                .bg(theme.background)
                                .ml(rems(0.375))
                                .gap_2()
                                .p_2()
                                .child(
                                    Icon::new(icon)
                                        .size(rems(1.0))
                                        .text_color(color)
                                        .flex_shrink_0(),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.foreground)
                                        .flex_1()
                                        .child(message),
                                )
                                .child(
                                    div()
                                        .id(("toast-close", id))
                                        .cursor_pointer()
                                        .flex_shrink_0()
                                        .child(
                                            Icon::new(IconName::Close)
                                                .size(rems(0.875))
                                                .text_color(theme.muted_foreground),
                                        )
                                        .on_click(move |_, _, cx| {
                                            close
                                                .update(cx, |this, cx| {
                                                    this.items.retain(|i| i.id != id);
                                                    cx.notify();
                                                })
                                                .ok();
                                        }),
                                ),
                        )
                        .into_any_element()
                })
                .collect()
        };

        div()
            .absolute()
            .bottom(rems(0.75))
            .left(rems(0.75))
            .w(rems(22.0))
            .child(v_flex().gap_2().children(rendered))
            .into_any_element()
    }
}
