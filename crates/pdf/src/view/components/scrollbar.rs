use crate::view::{PAGE_BASE_WIDTH_REMS, PdfReaderView, TOOLBAR_HEIGHT_REMS};
use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Styled,
    Window, div, px, relative, rems,
};
use gpui_component::ActiveTheme;

impl PdfReaderView {
    pub(crate) fn render_scrollbar(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        if self.total_pages > 0 {
            let scroll_top = self.list_state.logical_scroll_top();
            let current_ix = scroll_top.item_ix;

            let rem_size_px = f32::from(window.rem_size());

            // 计算总高度和当前绝对滚动位置（考虑多尺寸页面）
            let mut total_height_px = 0.0;
            let mut current_scroll_px = 0.0;

            for i in 0..self.total_pages {
                let (pdf_w, pdf_h) = self.page_sizes.get(i).copied().unwrap_or((612.0, 792.0));
                let page_h =
                    (PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size_px) * (pdf_h / pdf_w);

                if i < current_ix {
                    current_scroll_px += page_h;
                } else if i == current_ix {
                    current_scroll_px += f32::from(scroll_top.offset_in_item.abs());
                }

                total_height_px += page_h;
            }

            let toolbar_height = rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size());
            let tab_bar_h = self.tab_bar_offset_rems * f32::from(window.rem_size());
            let view_height_px =
                f32::from(window.viewport_size().height) - tab_bar_h - f32::from(toolbar_height);

            let scrollable_height_px = (total_height_px - view_height_px).max(0.0);

            let scroll_ratio = if scrollable_height_px > 0.0 {
                (current_scroll_px / scrollable_height_px).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let thumb_height_pct =
                (view_height_px / total_height_px.max(view_height_px)).clamp(0.05, 1.0);
            let track_avail_pct = 1.0 - thumb_height_pct;
            let thumb_top_pct = scroll_ratio * track_avail_pct;

            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(rems(1.0))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(
                        move |this: &mut Self,
                              event: &MouseDownEvent,
                              window: &mut Window,
                              cx: &mut Context<Self>| {
                            let toolbar_height =
                                rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size());
                            let tab_bar_h = this.tab_bar_offset_rems * f32::from(window.rem_size());
                            let content_height = f32::from(window.viewport_size().height)
                                - tab_bar_h
                                - f32::from(toolbar_height);
                            let thumb_height_px = content_height * thumb_height_pct;
                            this.drag_offset = thumb_height_px / 2.0;
                            this.is_dragging_scrollbar = true;
                            this.scroll_to_position(
                                event.position.y,
                                window.viewport_size().height,
                                window.rem_size(),
                                cx,
                            );
                        },
                    ),
                )
                .child(
                    div()
                        .absolute()
                        .right(px(2.0))
                        .top(relative(thumb_top_pct))
                        .w(px(6.0))
                        .h(relative(thumb_height_pct))
                        .bg(theme.scrollbar_thumb)
                        .rounded_full()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(
                                move |this: &mut Self,
                                      event: &MouseDownEvent,
                                      window: &mut Window,
                                      cx: &mut Context<Self>| {
                                    cx.stop_propagation();
                                    this.is_dragging_scrollbar = true;
                                    let toolbar_height =
                                        rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size());
                                    let tab_bar_h =
                                        this.tab_bar_offset_rems * f32::from(window.rem_size());
                                    let content_height = f32::from(window.viewport_size().height)
                                        - tab_bar_h
                                        - f32::from(toolbar_height);
                                    let mouse_y_rel = f32::from(event.position.y)
                                        - tab_bar_h
                                        - f32::from(toolbar_height);
                                    let thumb_top_px = content_height * thumb_top_pct;
                                    this.drag_offset = mouse_y_rel - thumb_top_px;
                                },
                            ),
                        ),
                )
        } else {
            div()
        }
    }
}
