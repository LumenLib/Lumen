use crate::view::PdfReaderView;
use crate::view::types::{PageColorMode, PdfIconName, TOOLBAR_HEIGHT_REMS, TranslationResult};
use crate::{AnnotationColor, AnnotationTool};
use gpui::prelude::*;
use gpui::{
    Context, Div, InteractiveElement, IntoElement, ParentElement, Styled, Window,
    WindowControlArea, div, px, rems,
};
#[cfg(not(target_os = "macos"))]
use gpui_component::Icon;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Selectable, h_flex, label::Label};
use i18n::I18nKey;

impl PdfReaderView {
    pub(crate) fn render_window_controls(
        &self,
        _window: &Window,
        _cx: &Context<Self>,
    ) -> impl IntoElement {
        #[cfg(not(target_os = "macos"))]
        {
            let theme = _cx.theme();
            let is_maximized = _window.is_maximized();

            h_flex()
                .h_full()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .id("window-minimize")
                        .h(rems(1.5))
                        .w(rems(1.5))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .cursor_pointer()
                        .occlude()
                        .window_control_area(WindowControlArea::Min)
                        .hover(|s| s.bg(theme.muted.opacity(0.6)))
                        .child(Icon::new(PdfIconName::Minimize).text_color(theme.foreground)),
                )
                .child(
                    div()
                        .id("window-maximize-restore")
                        .h(rems(1.5))
                        .w(rems(1.5))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_sm()
                        .cursor_pointer()
                        .occlude()
                        .window_control_area(WindowControlArea::Max)
                        .hover(|s| s.bg(theme.muted.opacity(0.6)))
                        .child(
                            Icon::new(if is_maximized {
                                PdfIconName::Restore
                            } else {
                                PdfIconName::Maximize
                            })
                            .text_color(theme.foreground),
                        ),
                )
                .child(
                    div()
                        .id("window-close")
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
                        .child(Icon::new(PdfIconName::Close).text_color(theme.foreground)),
                )
        }
        #[cfg(target_os = "macos")]
        div()
    }

    pub(crate) fn render_toolbar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        h_flex()
            .w_full()
            .h(rems(TOOLBAR_HEIGHT_REMS))
            .px_4()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.background)
            .items_center()
            .occlude()
            .child(
                // 左侧组：侧栏与搜索
                h_flex()
                    .gap_1()
                    .items_center()
                    .when(cfg!(target_os = "macos"), |this: Div| this.pl(px(80.0)))
                    .child(
                        Button::new("sidebar-toggle")
                            .ghost()
                            .icon(PdfIconName::Sidebar)
                            .tooltip(i18n::t(I18nKey::ToggleLeftSidebar, self.language))
                            .when(self.is_left_sidebar_open, |b| b.selected(true))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.is_left_sidebar_open = !this.is_left_sidebar_open;
                                this.apply_auto_fit(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(div().w_2()) // 间距
                    .child(
                        Button::new("tool-rectangle")
                            .ghost()
                            .icon(PdfIconName::Square)
                            .when(
                                matches!(
                                    self.annotation_state.active_tool,
                                    AnnotationTool::Rectangle(_)
                                ),
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if matches!(
                                    this.annotation_state.active_tool,
                                    AnnotationTool::Rectangle(_)
                                ) {
                                    this.annotation_state.active_tool = AnnotationTool::Select;
                                } else {
                                    this.annotation_state.active_tool =
                                        AnnotationTool::Rectangle(AnnotationColor::Red);
                                }
                                cx.notify();
                            }))
                            .tooltip(i18n::t(I18nKey::RectangleSelect, self.language)),
                    )
                    .child(
                        Button::new("tool-pin")
                            .ghost()
                            .icon(PdfIconName::Pin)
                            .when(
                                self.annotation_state.active_tool == AnnotationTool::Pin,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.annotation_state.active_tool == AnnotationTool::Pin {
                                    this.annotation_state.active_tool = AnnotationTool::Select;
                                } else {
                                    this.annotation_state.active_tool = AnnotationTool::Pin;
                                }
                                cx.notify();
                            }))
                            .tooltip("图钉"),
                    )
                    .child(div().w_2()) // 间距
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            // 经典白
                            .child(
                                div()
                                    .id("page-color-white")
                                    .w(rems(0.875))
                                    .h(rems(0.875))
                                    .rounded_full()
                                    .bg(gpui::white())
                                    .border_1()
                                    .border_color(if self.page_color_mode == PageColorMode::White {
                                        theme.accent
                                    } else {
                                        theme.border
                                    })
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_page_color_mode(PageColorMode::White, cx);
                                    })),
                            )
                            // 暖阳黄 / 羊皮纸
                            .child(
                                div()
                                    .id("page-color-sepia")
                                    .w(rems(0.875))
                                    .h(rems(0.875))
                                    .rounded_full()
                                    .bg(gpui::rgb(0xF4ECD8)) // F4ECD8
                                    .border_1()
                                    .border_color(if self.page_color_mode == PageColorMode::Sepia {
                                        theme.accent
                                    } else {
                                        theme.border
                                    })
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_page_color_mode(PageColorMode::Sepia, cx);
                                    })),
                            )
                            // 绿野护眼绿
                            .child(
                                div()
                                    .id("page-color-eye")
                                    .w(rems(0.875))
                                    .h(rems(0.875))
                                    .rounded_full()
                                    .bg(gpui::rgb(0xCCE8CF))
                                    .border_1()
                                    .border_color(
                                        if self.page_color_mode == PageColorMode::EyeProtect {
                                            theme.accent
                                        } else {
                                            theme.border
                                        },
                                    )
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_page_color_mode(PageColorMode::EyeProtect, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                // 弹性占位符 1 (左-中)
                div()
                    .flex_grow()
                    .h_full()
                    .occlude()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                // 中间组：阅读控制中心
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        // 导航胶囊
                        h_flex()
                            .gap_0()
                            .items_center()
                            .bg(theme.muted.opacity(0.3))
                            .rounded_lg()
                            .px_1()
                            .child(
                                Button::new("pdf-prev-btn")
                                    .ghost()
                                    .icon(PdfIconName::ChevronLeft)
                                    .on_click(cx.listener(|this, _, _, cx| this.prev_page(cx))),
                            )
                            .child(
                                div().px_4().child(
                                    Label::new(format!(
                                        "{} / {}",
                                        this_page_plus_one(self),
                                        self.total_pages
                                    ))
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM),
                                ),
                            )
                            .child(
                                Button::new("pdf-next-btn")
                                    .ghost()
                                    .icon(PdfIconName::ChevronRight)
                                    .on_click(cx.listener(|this, _, _, cx| this.next_page(cx))),
                            ),
                    )
                    .child(
                        // 缩放控制胶囊
                        h_flex()
                            .gap_0()
                            .items_center()
                            .bg(theme.muted.opacity(0.3))
                            .rounded_lg()
                            .px_1()
                            .child(
                                Button::new("zoom-out")
                                    .ghost()
                                    .icon(PdfIconName::ZoomOut)
                                    .on_click(cx.listener(|this, _, _, cx| this.zoom_out(cx)))
                                    .tooltip(i18n::t(I18nKey::ZoomOut, self.language)),
                            )
                            .child(
                                div().w(px(64.0)).child(
                                    Label::new(format!("{:.0}%", self.zoom_level * 100.0))
                                        .text_sm()
                                        .text_center()
                                        .font_weight(gpui::FontWeight::MEDIUM),
                                ),
                            )
                            .child(
                                Button::new("zoom-in")
                                    .ghost()
                                    .icon(PdfIconName::ZoomIn)
                                    .on_click(cx.listener(|this, _, _, cx| this.zoom_in(cx)))
                                    .tooltip(i18n::t(I18nKey::ZoomIn, self.language)),
                            )
                            .child(
                                Button::new("reset-zoom")
                                    .ghost()
                                    .icon(PdfIconName::FitWidth)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.reset_zoom(window, cx)
                                    }))
                                    .tooltip(i18n::t(I18nKey::FitWidth, self.language)),
                            ),
                    ),
            )
            .child(
                // 弹性占位符 2 (中-右)
                div()
                    .flex_grow()
                    .h_full()
                    .occlude()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                // 右侧组：功能侧栏
                h_flex().items_center().child(
                    Button::new("right-sidebar-toggle")
                        .ghost()
                        .icon(PdfIconName::PanelRight)
                        .when(self.is_right_sidebar_open, |b| b.selected(true))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.is_right_sidebar_open = !this.is_right_sidebar_open;
                            this.apply_auto_fit(window, cx);
                            if this.is_right_sidebar_open
                                && let Some(text) = this.selected_text.clone()
                            {
                                if this.auto_translate {
                                    this.translate_text(text, cx);
                                } else {
                                    this.translation_result = Some(TranslationResult {
                                        original: text.clone(),
                                        translated: None,
                                        is_loading: false,
                                        error: None,
                                    });
                                    cx.notify();
                                }
                            }
                            cx.notify();
                        }))
                        .tooltip(i18n::t(I18nKey::ToggleRightSidebar, self.language)),
                ),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .mr_2()
                    .child(self.render_window_controls(window, cx)),
            )
    }
}

fn this_page_plus_one(view: &PdfReaderView) -> u16 {
    view.current_page + 1
}
