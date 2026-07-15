use crate::services::MainApp;
use gpui::prelude::*;
use gpui::{AnyElement, App, FontWeight, SharedString, Window, div};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use i18n::{I18nKey, t};
use models::Literature;
use parser::normalize::author_full_name;
use std::sync::Arc;

type DuplicateComplete = Box<dyn FnOnce(Option<usize>, &mut Window, &mut App) + 'static>;

pub struct DuplicateListDialogContent {
    app: Arc<MainApp>,
    groups: Vec<Vec<Literature>>,
    conflict_mode: bool,
    on_complete: Option<DuplicateComplete>,
}

impl DuplicateListDialogContent {
    pub fn new(app: Arc<MainApp>, groups: Vec<Vec<Literature>>, conflict_mode: bool) -> Self {
        Self {
            app,
            groups,
            conflict_mode,
            on_complete: None,
        }
    }

    pub fn set_on_complete(&mut self, cb: DuplicateComplete) {
        self.on_complete = Some(cb);
    }
}

impl Render for DuplicateListDialogContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        v_flex().size_full().child(
            v_flex()
                .gap_3()
                .when(self.groups.is_empty(), |this| {
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

                    let card_content: AnyElement = if self.conflict_mode && group.len() >= 2 {
                        render_conflict_card(&theme, lang, group).into_any_element()
                    } else {
                        render_duplicate_card(&theme, lang, group).into_any_element()
                    };

                    let this_handle = cx.entity().downgrade();
                    div()
                        .id(group_id)
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme.border)
                        .hover(|s| s.bg(theme.accent).text_color(theme.accent_foreground))
                        .cursor_pointer()
                        .on_click(move |_, window, cx| {
                            if let Some(this) = this_handle.upgrade() {
                                this.update(cx, |this, cx| {
                                    if let Some(cb) = this.on_complete.take() {
                                        cb(Some(idx), window, cx);
                                    }
                                });
                            }
                        })
                        .child(card_content)
                })),
        )
    }
}

fn render_duplicate_card(
    theme: &gpui_component::Theme,
    _lang: i18n::Language,
    group: &[Literature],
) -> impl IntoElement {
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
                .child(format!("{} 条目", group.len())),
        )
}

fn render_conflict_card(
    theme: &gpui_component::Theme,
    lang: i18n::Language,
    group: &[Literature],
) -> impl IntoElement {
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
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(t(I18nKey::LocalData, lang)),
                )
                .child(div().font_weight(FontWeight::BOLD).child(local_title)),
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
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(t(I18nKey::RemoteData, lang)),
                )
                .child(div().font_weight(FontWeight::BOLD).child(remote_title)),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .px_2()
                .child(remote_authors),
        )
}
