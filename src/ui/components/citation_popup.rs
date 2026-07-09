use crate::services::MainApp;
use crate::ui::components::muted_select;
use crate::ui::icons::IconName;
use gpui::SharedString;
use gpui::prelude::*;
use gpui::{AppContext, ClipboardItem, Entity, FontWeight, Window, WindowControlArea, div, rems};
use gpui_component::{
    ActiveTheme, Disableable, Icon, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectItem, SelectState},
    v_flex,
};
use i18n::{I18nKey, t};
use models::Literature;
use parser::csl::StyleInfo;
use parser::export::ExportFormat;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone)]
struct StyleInfoItem(StyleInfo);

impl From<StyleInfo> for StyleInfoItem {
    fn from(s: StyleInfo) -> Self {
        Self(s)
    }
}

impl SelectItem for StyleInfoItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.0.name.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0.id
    }
}

pub struct CitationPopup {
    app: Arc<MainApp>,
    selected_ids: HashSet<String>,
    style_select: Entity<SelectState<Vec<StyleInfoItem>>>,
    selected_style: String,
    citation_text: String,
}

impl CitationPopup {
    pub fn new(
        app: Arc<MainApp>,
        selected_ids: HashSet<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut styles = app.available_citation_styles();
        // 添加 BibTeX 选项
        styles.insert(
            0,
            StyleInfo {
                id: "bibtex".to_string(),
                name: "BibTeX".to_string(),
            },
        );

        // 尝试从本地加载的样式中寻找默认值，如果没有则取第一个
        let default_style = if styles.iter().any(|s| s.id == "ieee") {
            "ieee".to_string()
        } else {
            styles
                .first()
                .map_or_else(|| "bibtex".to_string(), |s| s.id.clone())
        };

        // 初始化 SelectState
        let style_items: Vec<StyleInfoItem> = styles.into_iter().map(StyleInfoItem::from).collect();
        let style_select = cx.new(|cx| {
            let mut state = SelectState::new(style_items, None, window, cx);
            state.set_selected_value(&default_style, window, cx);
            state
        });

        // 订阅选择事件
        cx.subscribe(&style_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(style_id)) = event {
                this.handle_style_select(style_id.clone(), cx);
            }
        })
        .detach();

        let mut this = Self {
            app: app.clone(),
            selected_ids,
            style_select,
            selected_style: default_style.clone(),
            citation_text: String::new(),
        };

        this.update_citation(cx);
        this
    }

    fn update_citation(&mut self, cx: &mut Context<Self>) {
        let lang = self.app.current_language();
        let style = self.selected_style.clone();

        if style == "bibtex" {
            let all_lits = self.app.db.get_all_literatures().unwrap_or_default();
            let selected_lits: Vec<Literature> = all_lits
                .iter()
                .filter(|l| self.selected_ids.contains(&l.id))
                .cloned()
                .collect();

            if selected_lits.is_empty() {
                self.citation_text = t(I18nKey::NoLiteratureSelectedForCitation, lang).to_string();
            } else {
                match self
                    .app
                    .export_manager
                    .export_to_string(ExportFormat::BibTeX, &selected_lits)
                {
                    Ok(text) => self.citation_text = text,
                    Err(e) => {
                        self.citation_text = format!("{}: {}", t(I18nKey::CitationError, lang), e);
                    }
                }
            }
        } else {
            match self
                .app
                .format_selected_literatures(&self.selected_ids, &style)
            {
                Ok(text) => {
                    self.citation_text = text;
                }
                Err(e) => {
                    self.citation_text = format!("{}: {}", t(I18nKey::CitationError, lang), e);
                }
            }
        }
        cx.notify();
    }

    fn handle_copy(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.citation_text.is_empty() || self.citation_text.starts_with("错误:") {
            return;
        }

        cx.write_to_clipboard(ClipboardItem::new_string(self.citation_text.clone()));
    }

    fn handle_style_select(&mut self, style_id: String, cx: &mut Context<Self>) {
        if self.selected_style != style_id {
            self.selected_style = style_id;
            self.update_citation(cx);
        }
    }
}

impl Render for CitationPopup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();
        let background = theme.background;
        let text = theme.foreground;
        let border = theme.border;
        let muted = theme.muted;
        let is_disabled = self.citation_text.is_empty()
            || self
                .citation_text
                .starts_with(t(I18nKey::CitationError, lang))
            || self.citation_text == t(I18nKey::NoLiteratureSelectedForCitation, lang);

        v_flex()
            .size_full()
            .bg(background)
            .p_6()
            .gap_5()
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
                                .id("citation-modal-close-btn")
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
                h_flex().w_full().justify_between().items_center().child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(text)
                        .child(t(I18nKey::CopyCitationTitle, lang)),
                ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(text)
                            .child(t(I18nKey::Style, lang)),
                    )
                    .child(div().w(rems(18.75)).child(
                        muted_select(Select::new(&self.style_select).placeholder(t(I18nKey::Style, lang)), &theme),
                    )),
            )
            .child(
                v_flex()
                    .gap_2()
                    .flex_grow()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .text_color(text)
                            .child(t(I18nKey::Preview, lang)),
                    )
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .child(
                                div()
                                    .size_full()
                                    .border_1()
                                    .border_color(border)
                                    .rounded_md()
                                    .p_4()
                                    .bg(muted)
                                    .overflow_y_scrollbar()
                                    .child(
                                        div()
                                            .w_full()
                                            .pr_8()
                                            .text_sm()
                                            .text_color(text)
                                            .child(self.citation_text.clone()),
                                    ),
                            )
                            .child(
                                div().absolute().top_2().right_2().child(
                                    Button::new("copy-icon-btn")
                                        .ghost()
                                        .disabled(is_disabled)
                                        .child(Icon::new(IconName::Copy).size(rems(0.875)))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.handle_copy(window, cx);
                                        })),
                                ),
                            ),
                    ),
            )
    }
}
