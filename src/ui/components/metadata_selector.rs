use components::{IconName, add_drag_behavior};
use gpui::prelude::*;
use gpui::{ElementId, FontWeight, SharedString, Window, div, px, rems};
use gpui_component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollableElement,
    v_flex,
};
use i18n::{I18nKey, t};
use models::Literature;
use parser::normalize::author_full_name;
use services::app::MainApp;
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        v_flex()
            .relative()
            .size_full()
            .shadow_md()
            .bg(theme.background)
            .rounded_xl()
            .px_6()
            .pt(px(10.0))
            .pb_6()
            .border_1()
            .border_color(theme.border)
            .gap_4()
            .child(add_drag_behavior(
                div()
                    .id("meta-sel-drag-overlay")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(40.0)),
                window,
                cx,
            ))
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::SelectMetadataCandidate, lang)),
                    )
                    .child(
                        Button::new("meta-sel-close")
                            .ghost()
                            .child(Icon::new(IconName::Close).size(rems(0.875)))
                            .on_click(cx.listener(
                                |_, _, window: &mut Window, _: &mut Context<Self>| {
                                    window.remove_window();
                                },
                            )),
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
