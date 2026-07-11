use crate::AnnotationTool;
use crate::view::PdfReaderView;
use crate::view::types::{PageColorMode, PdfIconName, TOOLBAR_HEIGHT_REMS, TranslationResult};
use gpui::prelude::*;
use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Styled, Window, WindowControlArea,
    div, px, rems,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Disableable, Selectable, h_flex, label::Label};

impl PdfReaderView {
    pub(crate) fn render_toolbar(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = cx.theme();
        let border = t.border;
        let muted = t.muted;
        let background = t.background;

        h_flex()
            .w_full()
            .h(rems(TOOLBAR_HEIGHT_REMS))
            .px_4()
            .border_b_1()
            .border_color(border)
            .bg(background)
            .items_center()
            .occlude()
            // ─── 1. 左侧组：侧栏开关 + 颜色小白点 ───
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        Button::new("sidebar-toggle")
                            .ghost()
                            .icon(PdfIconName::Sidebar)
                            .h(rems(1.4))
                            .w(rems(1.4))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.is_left_sidebar_open = !this.is_left_sidebar_open;
                                this.apply_auto_fit(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(self.render_color_dot(PageColorMode::White, border, cx))
                            .child(self.render_color_dot(PageColorMode::Sepia, border, cx))
                            .child(self.render_color_dot(
                                PageColorMode::EyeProtect,
                                border,
                                cx,
                            )),
                    ),
            )
            .child(
                // 弹性占位符 1 (左-中)
                div()
                    .flex_grow(1.0)
                    .h_full()
                    .occlude()
                    .window_control_area(WindowControlArea::Drag),
            )
            // ─── 2. 正中间组：核心标注工具 (仅框选与Pin) ───
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .child(
                        Button::new("tool-rectangle")
                            .ghost()
                            .icon(PdfIconName::Square)
                            .h(rems(1.4))
                            .w(rems(1.4))
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
                                    this.annotation_state.active_tool = AnnotationTool::Rectangle(
                                        this.annotation_state.last_highlight_color,
                                    );
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tool-pin")
                            .ghost()
                            .icon(PdfIconName::Pin)
                            .h(rems(1.4))
                            .w(rems(1.4))
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
                            })),
                    ),
            )
            .child(
                // 弹性占位符 2 (中-右)
                div()
                    .flex_grow(1.0)
                    .h_full()
                    .occlude()
                    .window_control_area(WindowControlArea::Drag),
            )
            // ─── 3. 右侧组：缩放 + 自适应 + 翻页导航 + 右侧栏开关 ───
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(
                        // 缩放控制胶囊
                        h_flex()
                            .gap_0()
                            .items_center()
                            .bg(muted.opacity(0.3))
                            .rounded_lg()
                            .px_1()
                            .child(
                                Button::new("zoom-out")
                                    .ghost()
                                    .icon(PdfIconName::ZoomOut)
                                    .h(rems(1.3))
                                    .w(rems(1.3))
                                    .on_click(cx.listener(|this, _, _, cx| this.zoom_out(cx))),
                            )
                            .child(
                                div().w(px(64.0)).child(
                                    Label::new(format!("{:.0}%", self.zoom_level * 100.0))
                                        .text_xs()
                                        .text_center()
                                        .font_weight(gpui::FontWeight::MEDIUM),
                                ),
                            )
                            .child(
                                Button::new("zoom-in")
                                    .ghost()
                                    .icon(PdfIconName::ZoomIn)
                                    .h(rems(1.3))
                                    .w(rems(1.3))
                                    .on_click(cx.listener(|this, _, _, cx| this.zoom_in(cx))),
                            ),
                    )
                    .child(
                        // 自适应窗口按钮放在缩放右侧
                        Button::new("reset-zoom")
                            .ghost()
                            .icon(PdfIconName::FitWidth)
                            .h(rems(1.4))
                            .w(rems(1.4))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.reset_zoom(window, cx)),
                            ),
                    )
                    .child(
                        // 翻页胶囊：[‹] 当前/总页 [›]
                        h_flex()
                            .gap_0()
                            .items_center()
                            .bg(muted.opacity(0.3))
                            .rounded_lg()
                            .px_1()
                            .child(
                                Button::new("pdf-prev")
                                    .ghost()
                                    .icon(PdfIconName::ChevronLeft)
                                    .h(rems(1.4))
                                    .w(rems(1.4))
                                    .disabled(self.current_page == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scroll_to_page(
                                            this.current_page.saturating_sub(1),
                                            px(0.0),
                                            cx,
                                        );
                                    })),
                            )
                            .child(
                                div().px_2().child(
                                    Label::new(format!(
                                        "{} / {}",
                                        self.current_page + 1,
                                        self.total_pages
                                    ))
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::MEDIUM),
                                ),
                            )
                            .child(
                                Button::new("pdf-next")
                                    .ghost()
                                    .icon(PdfIconName::ChevronRight)
                                    .h(rems(1.4))
                                    .w(rems(1.4))
                                    .disabled(
                                        self.total_pages == 0
                                            || self.current_page as usize + 1 >= self.total_pages,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.scroll_to_page(
                                            this.current_page.saturating_add(1),
                                            px(0.0),
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        Button::new("right-sidebar-toggle")
                            .ghost()
                            .icon(PdfIconName::PanelRight)
                            .h(rems(1.4))
                            .w(rems(1.4))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.is_right_sidebar_open = !this.is_right_sidebar_open;
                                this.apply_auto_fit(window, cx);
                                if this.is_right_sidebar_open
                                    && let Some(text) = this.selected_text.clone()
                                {
                                    if this.auto_translate {
                                        this.translate_text(text, false, cx);
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
                            })),
                    ),
            )
    }

    fn render_color_dot(
        &self,
        mode: PageColorMode,
        border: gpui::Hsla,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id_str = match mode {
            PageColorMode::White => "page-color-white",
            PageColorMode::Sepia => "page-color-sepia",
            PageColorMode::EyeProtect => "page-color-eye",
        };
        let is_active = self.page_color_mode == mode;
        div()
            .id(id_str)
            .w(rems(0.875))
            .h(rems(0.875))
            .rounded_full()
            .bg(mode.bg_color())
            .border_2()
            .border_color(if is_active { gpui::white() } else { border })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_page_color_mode(mode, cx);
            }))
    }
}
