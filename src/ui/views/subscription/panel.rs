use crate::services::MainApp;
use crate::services::data_store::DataStore;
use crate::ui::views::main_window::{ContextMenuType, MainWindow};
use components::IconName;
use gpui::prelude::*;
use gpui::{
    AnyElement, Entity, Hsla, MouseButton, MouseDownEvent, SharedString, WeakEntity, Window, div,
    rems,
};
use gpui_component::{ActiveTheme, Icon, Sizable, Theme, h_flex};
use i18n::{I18nKey, t};
use models::Feed;
use std::sync::Arc;

pub struct SubscriptionPanel {
    app: Arc<MainApp>,
    data_store: Entity<DataStore>,
    parent_view: WeakEntity<MainWindow>,
}

impl SubscriptionPanel {
    pub fn new(
        app: Arc<MainApp>,
        data_store: Entity<DataStore>,
        parent_view: WeakEntity<MainWindow>,
    ) -> Self {
        Self {
            app,
            data_store,
            parent_view,
        }
    }

    pub fn select_feed(&mut self, feed_id: String, cx: &mut Context<Self>) {
        let parent = self.parent_view.clone();
        let _ = parent.update(cx, |mw, mw_cx| mw.select_feed(feed_id, mw_cx));
    }

    fn render_static_item(
        &self,
        props: StaticItemProps,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id_str = props.id.clone();
        let color = if props.is_selected {
            props.theme.primary_foreground
        } else {
            props.theme.foreground
        };
        let id_str_right = id_str.clone();
        let id_str_click = id_str.clone();

        div()
            .id(SharedString::from(format!("static-item-{}", props.id)))
            .py_1()
            .px_3()
            .mx_2()
            .flex()
            .items_center()
            .rounded_md()
            .when(props.is_selected, |s| {
                s.bg(props.theme.primary)
                    .text_color(props.theme.primary_foreground)
            })
            .when(!props.is_selected, |s| {
                s.hover(|s| s.bg(props.theme.primary.opacity(0.15)))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.select_feed(id_str_right.clone(), cx);
                }),
            )
            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                this.select_feed(id_str_click.clone(), cx);
                cx.notify();
            }))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        h_flex().gap_2().child((props.icon_builder)(color)).child(
                            div()
                                .text_sm()
                                .text_color(if props.is_selected {
                                    props.theme.primary_foreground
                                } else {
                                    props.theme.foreground
                                })
                                .child(props.text),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if props.is_selected {
                                props.theme.primary_foreground
                            } else {
                                props.theme.muted_foreground
                            })
                            .child(props.count),
                    ),
            )
    }

    fn render_feed_item(
        &self,
        feed: Arc<Feed>,
        selected_id: Option<String>,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = selected_id.as_ref() == Some(&feed.id);
        let feed_id = feed.id.clone();
        let feed_id_right = feed.id.clone();

        let parent = self.parent_view.clone();

        let theme_selected = theme.clone();
        let theme_hover = theme.clone();
        let theme_icon = theme.clone();

        div()
            .id(SharedString::from(format!("feed-item-{}", feed.id)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let feed_id = feed_id_right.clone();
                    let parent = parent.clone();
                    move |this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        // 1. 先选中
                        this.select_feed(feed_id.clone(), cx);
                        // 2. 再显示菜单
                        if let Some(mw) = parent.upgrade() {
                            mw.update(cx, |mw, cx| {
                                mw.show_context_menu(
                                    event.position,
                                    ContextMenuType::Subscription(Some(feed_id.clone())),
                                    window,
                                    cx,
                                );
                            });
                        }
                    }
                }),
            )
            .on_click(cx.listener({
                let feed_id = feed_id.clone();

                move |this: &mut Self, _, _, cx| {
                    this.select_feed(feed_id.clone(), cx);

                    cx.notify();
                }
            }))
            .flex()
            .items_center()
            .px_3()
            .py_0p5()
            .mx_2()
            .rounded_md()
            .hover(move |s| s.bg(theme_hover.primary.opacity(0.15)))
            .when(is_selected, move |s| {
                s.bg(theme_selected.primary)
                    .text_color(theme_selected.primary_foreground)
            })
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Icon::new(IconName::Globe)
                                    .small()
                                    .text_color(if is_selected {
                                        theme_icon.primary_foreground
                                    } else {
                                        theme_icon.foreground
                                    }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(if is_selected {
                                        theme_icon.primary_foreground
                                    } else {
                                        theme_icon.foreground
                                    })
                                    .child(feed.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if is_selected {
                                theme_icon.primary_foreground
                            } else {
                                theme_icon.muted_foreground
                            })
                            .child(feed.total_count.to_string()),
                    ),
            )
    }
}

impl Render for SubscriptionPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut feeds = self.data_store.read(cx).feeds.clone();

        // 按名称排序 Feed
        feeds.sort_by(|a, b| {
            // 将 static items (all_subs, unread) 保持在原位或通过 loop logic 处理，
            // 这里的 feeds 包含所有 feeds，所以我们需要对非 static items 排序
            if a.id == "all_subs" || a.id == "unread" {
                std::cmp::Ordering::Less
            } else if b.id == "all_subs" || b.id == "unread" {
                std::cmp::Ordering::Greater
            } else {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
        });

        let lang = self.app.current_language();
        let ui = cx.global::<crate::services::ui_state::UiState>();
        let selected_feed_id = ui.selected_feed_id.clone();
        let parent_view = self.parent_view.clone();
        let theme = cx.theme().clone();

        div()
            .flex()
            .flex_col()
            .w_full()
            .flex_grow(1.0)
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .relative()
            .child({
                let all_subs_feed = feeds.iter().find(|f| f.id == "all_subs");
                let unread_feed = feeds.iter().find(|f| f.id == "unread");

                let all_total = all_subs_feed.map_or(0, |f| f.total_count);
                let unread_total = unread_feed.map_or(0, |f| f.unread_count);

                div()
                    .flex()
                    .flex_col()
                    .flex_grow(1.0)
                    .min_h_0()
                    .child(self.render_static_item(
                        StaticItemProps {
                            icon_builder: Box::new(|color| {
                                Icon::new(IconName::Globe)
                                    .small()
                                    .text_color(color)
                                    .into_any_element()
                            }),
                            text: t(I18nKey::AllSubscription, lang).to_string(),
                            count: all_total.to_string(),
                            is_selected: selected_feed_id.as_ref() == Some(&"all_subs".to_string()),
                            id: "all_subs".to_string(),
                            theme: theme.clone(),
                        },
                        cx,
                    ))
                    .child(self.render_static_item(
                        StaticItemProps {
                            icon_builder: Box::new(|color| {
                                Icon::new(IconName::Bell)
                                    .small()
                                    .text_color(color)
                                    .into_any_element()
                            }),
                            text: t(I18nKey::Unread, lang).to_string(),
                            count: unread_total.to_string(),
                            is_selected: selected_feed_id.as_ref() == Some(&"unread".to_string()),
                            id: "unread".to_string(),
                            theme: theme.clone(),
                        },
                        cx,
                    ))
                    .child(div().h(rems(0.0625)).bg(theme.border).my_2().mx_4())
                    .child({
                        let parent = parent_view.clone();
                        div()
                            .id("feed-list")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .on_mouse_down(
                                MouseButton::Right,
                                move |event: &MouseDownEvent, window, cx| {
                                    // 空白区域触发"添加订阅"菜单
                                    if let Some(mw) = parent.upgrade() {
                                        mw.update(cx, |mw, cx| {
                                            mw.show_context_menu(
                                                event.position,
                                                ContextMenuType::Subscription(None),
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                },
                            )
                            .children(
                                feeds
                                    .into_iter()
                                    .filter(|f| f.id != "all_subs" && f.id != "unread")
                                    .map(|feed| {
                                        self.render_feed_item(
                                            feed,
                                            selected_feed_id.clone(),
                                            theme.clone(),
                                            cx,
                                        )
                                        .into_any_element()
                                    }),
                            )
                            .child(div().h(rems(6.25)).w_full().flex_shrink_0())
                    })
            })
    }
}

struct StaticItemProps {
    icon_builder: Box<dyn Fn(Hsla) -> AnyElement>,
    text: String,
    count: String,
    is_selected: bool,
    id: String,
    theme: Theme,
}
