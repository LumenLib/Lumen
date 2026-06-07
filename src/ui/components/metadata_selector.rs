use crate::services::MainApp;
use crate::ui::icons::IconName;
use gpui::prelude::*;
use gpui::{ElementId, FontWeight, SharedString, Window, WindowControlArea, div, rems};
use gpui_component::{ActiveTheme, Icon, h_flex, scroll::ScrollableElement, v_flex};
use i18n::{I18nKey, t};
use models::Literature;
use parser::normalize::author_full_name;
use std::sync::Arc;

pub type MetadataSelectorCallback =
    Box<dyn Fn(Option<Literature>, &mut Window, &mut Context<MetadataSelector>) + Send + Sync>;

pub struct MetadataSelector {
    app: Arc<MainApp>,
    candidates: Vec<Arc<Literature>>,
    on_complete: MetadataSelectorCallback,
}

impl MetadataSelector {
    pub fn new(
        app: Arc<MainApp>,
        candidates: Vec<Arc<Literature>>,
        on_complete: impl Fn(Option<Literature>, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            app,
            candidates,
            on_complete: Box::new(on_complete),
        }
    }
}

impl Render for MetadataSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        v_flex()
            .size_full()
            .shadow_md()
            .bg(theme.background)
            .rounded_xl()
            .px_6()
            .pt(rems(2.0))
            .pb_6()
            .border_1()
            .border_color(theme.border)
            .gap_4()
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
                                .id("meta-sel-modal-close-btn")
                                .h(rems(1.5))
                                .w(rems(1.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .cursor_pointer()
                                .occlude()
                                .window_control_area(WindowControlArea::Close)
                                .hover(|s| s.bg(gpui::red().opacity(0.9)))
                                .child(
                                    Icon::new(IconName::Close)
                                        .size(rems(0.875))
                                        .text_color(theme.foreground),
                                ),
                        ),
                )
            })
            .child(
                h_flex().justify_between().items_center().child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .child(t(I18nKey::SelectMetadataCandidate, lang)),
                ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0() // 关键修复：允许 Flex 子项收缩，从而触发滚动条
                    .overflow_y_scrollbar()
                    .gap_2()
                    .children(self.candidates.iter().enumerate().map(|(idx, item)| {
                        let authors = item
                            .authors
                            .iter()
                            .map(author_full_name)
                            .take(3)
                            .collect::<Vec<_>>()
                            .join(", ");

                        let authors_display = if item.authors.len() > 3 {
                            format!("{authors}...")
                        } else {
                            authors
                        };

                        let year = item.year.map(|y| y.to_string()).unwrap_or_default();
                        let container_title = item
                            .publication
                            .as_ref()
                            .map(|p| p.name.clone())
                            .unwrap_or_default();

                        let meta_info = if !year.is_empty() && !container_title.is_empty() {
                            format!("{year} - {container_title}")
                        } else {
                            format!("{year}{container_title}")
                        };

                        let item_clone: Literature = (**item).clone();

                        div()
                            .id(ElementId::from(SharedString::from(format!(
                                "candidate-{idx}"
                            ))))
                            .p_3()
                            .rounded_md()
                            .border_1()
                            .border_color(theme.border)
                            .hover(|s| s.bg(theme.muted))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                (this.on_complete)(Some(item_clone.clone()), window, cx);
                            }))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::BOLD)
                                            .overflow_hidden()
                                            .text_ellipsis()
                                            .whitespace_nowrap()
                                            .child(item.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(authors_display),
                                    )
                                    .child(
                                        div().text_xs().text_color(theme.primary).child(meta_info),
                                    ),
                            )
                    })),
            )
    }
}
