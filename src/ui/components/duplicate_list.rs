use crate::services::MainApp;
use crate::ui::icons::IconName;
use crate::ui::theme_manager::surface;
use gpui::prelude::*;
use gpui::{ElementId, FontWeight, SharedString, Window, WindowControlArea, div, rems};
use gpui_component::{ActiveTheme, Icon, h_flex, scroll::ScrollableElement, v_flex};
use i18n::{I18nKey, t};
use models::Literature;
use parser::normalize::author_full_name;
use std::sync::Arc;

pub type DuplicateListCallback =
    Box<dyn Fn(Option<usize>, &mut Window, &mut Context<DuplicateList>) + Send + Sync>;

pub struct DuplicateList {
    app: Arc<MainApp>,
    groups: Vec<Vec<Literature>>,
    on_complete: DuplicateListCallback,
    conflict_mode: bool,
}

impl DuplicateList {
    pub fn new(
        app: Arc<MainApp>,
        groups: Vec<Vec<Literature>>,
        on_complete: impl Fn(Option<usize>, &mut Window, &mut Context<Self>) + Send + Sync + 'static,
        conflict_mode: bool,
    ) -> Self {
        Self {
            app,
            groups,
            on_complete: Box::new(on_complete),
            conflict_mode,
        }
    }
}

impl Render for DuplicateList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        v_flex()
            .size_full()
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
                // Window controls
                .child(
                    div()
                        .absolute()
                        .top_1()
                        .right_1()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id("dup-list-modal-close-btn")
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
                // 标题栏
                h_flex()
                    .px_6()
                    .pt(rems(2.0))
                    .pb_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .justify_between()
                    .items_center()
                    .child(div().text_base().font_weight(FontWeight::BOLD).child(t(
                        if self.conflict_mode {
                            I18nKey::SyncConflicts
                        } else {
                            I18nKey::DuplicateGroups
                        },
                        lang,
                    ))),
            )
            .child(
                // 列表区域
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .px_6()
                    .py_3()
                    .child(
                        v_flex()
                            .gap_3()
                            .when(self.groups.is_empty(), |this: gpui::Div| {
                                this.child(
                                    div()
                                        .py_10()
                                        .text_center()
                                        .text_sm()
                                        .text_color(theme.muted_foreground)
                                        .child(t(I18nKey::NoDuplicatesFound, lang)),
                                )
                            })
                            .children(self.groups.iter().enumerate().map(|(idx, group)| {
                                let group_id = SharedString::from(format!("duplicate-group-{idx}"));

                                let card_content = if self.conflict_mode && group.len() >= 2 {
                                    let local = &group[0];
                                    let remote = &group[1];
                                    let local_title = if local.title.trim().is_empty() {
                                        SharedString::from("Untitled")
                                    } else {
                                        SharedString::from(local.title.clone())
                                    };
                                    let remote_title = if remote.title.trim().is_empty() {
                                        SharedString::from("Untitled")
                                    } else {
                                        SharedString::from(remote.title.clone())
                                    };
                                    let local_authors = local
                                        .authors
                                        .iter()
                                        .map(author_full_name)
                                        .collect::<Vec<_>>()
                                        .join(", ");
                                    let remote_authors = remote
                                        .authors
                                        .iter()
                                        .map(author_full_name)
                                        .collect::<Vec<_>>()
                                        .join(", ");

                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.primary)
                                                .child(t(I18nKey::SyncConflicts, lang)),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(t(I18nKey::LocalData, lang)),
                                                )
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(local_title),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .px_2()
                                                .child(local_authors),
                                        )
                                        .child(div().border_b_1().border_color(theme.border).my_1())
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(theme.muted_foreground)
                                                        .child(t(I18nKey::RemoteData, lang)),
                                                )
                                                .child(
                                                    div()
                                                        .font_weight(FontWeight::BOLD)
                                                        .child(remote_title),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .px_2()
                                                .child(remote_authors),
                                        )
                                        .into_any_element()
                                } else {
                                    let first = &group[0];
                                    let title = if first.title.trim().is_empty() {
                                        SharedString::from("Untitled")
                                    } else {
                                        SharedString::from(first.title.clone())
                                    };
                                    let authors = first
                                        .authors
                                        .iter()
                                        .map(author_full_name)
                                        .collect::<Vec<_>>()
                                        .join(", ");

                                    v_flex()
                                        .gap_1()
                                        .child(div().font_weight(FontWeight::BOLD).child(title))
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(authors),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.primary)
                                                .child(format!("{} items", group.len())),
                                        )
                                        .into_any_element()
                                };

                                div()
                                    .id(ElementId::from(group_id))
                                    .p_4()
                                    .rounded_lg()
                                    .border_1()
                                    .border_color(theme.border)
                                    .hover(|s| {
                                        s.bg(theme.accent).text_color(theme.accent_foreground)
                                    })
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        (this.on_complete)(Some(idx), window, cx);
                                    }))
                                    .child(card_content)
                            })),
                    ),
            )
    }
}
