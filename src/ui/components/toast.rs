use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    AnyElement, AsyncApp, Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    Styled, Window, div, px, rems,
};
use gpui_component::notification::NotificationType;
use gpui_component::{ActiveTheme, Icon, h_flex, v_flex};

use crate::notification_bus::NotificationBus;
use components::IconName;

/// 在消息每个字符后插入零宽空格 (U+200B)，使 GPUI 断行器可在任意位置断行。
///
/// GPUI 的 `WhiteSpace` 仅有 `Normal`/`Nowrap`，无 `overflow-wrap: break-word`；
/// 其断行器只在空格后或 CJK 字符间设断点，连续 word 字符（如 `sha256_password`、
/// URL、哈希）中间无任何断点，会撑破容器。U+200B 属于非 word 字符，断行器会把它
/// 前后的位置都视为断点，从而让任意长 token 都能在行尾被切开。
fn break_anywhere(message: &str) -> SharedString {
    let mut out = String::with_capacity(message.len() * 2);
    for c in message.chars() {
        out.push(c);
        out.push('\u{200B}');
    }
    SharedString::from(out)
}

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
                        .w_full()
                        .max_w(rems(24.0))
                        .overflow_hidden()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.background)
                        .shadow_lg()
                        .occlude()
                        .gap_2()
                        .p_2()
                        .child(
                            // 左侧色条：替代原嵌套 h_flex 背景层，使整个 Toast 只需一个容器
                            div()
                                .w(px(3.))
                                .h_full()
                                .rounded_full()
                                .bg(color)
                                .flex_shrink_0(),
                        )
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
                                .min_w_0()
                                .whitespace_normal()
                                .child(break_anywhere(&message)),
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
                        )
                        .into_any_element()
                })
                .collect()
        };

        div()
            .absolute()
            .bottom(rems(0.75))
            .left(rems(0.75))
            .max_w(rems(24.0))
            .child(v_flex().gap_2().children(rendered))
            .into_any_element()
    }
}
