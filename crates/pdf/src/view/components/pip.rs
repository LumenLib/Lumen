use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Context, ImageSource, InteractiveElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, Size, Styled, Window, div, img, px,
};
use log::debug;

/// 画中画图钉
pub struct PiPPin {
    pub id: String,
    /// 源页面
    pub page: u16,
    /// PDF 坐标中的区域 (x0, y0, x1, y1)
    pub bbox: (f32, f32, f32, f32),
    /// 当前显示位置（屏幕坐标）
    pub position: Point<Pixels>,
    /// 当前显示尺寸
    pub size: Size<Pixels>,
    /// 当前渲染的图像（None 表示正在加载）
    pub image_source: Option<ImageSource>,
}

#[derive(Clone)]
pub(crate) struct PiPDragState {
    pub pin_id: String,
    pub offset: Point<Pixels>,
}

#[derive(Clone)]
pub(crate) struct PiPResizeState {
    pub pin_id: String,
    pub start_mouse: Point<Pixels>,
    pub start_bounds: Bounds<f32>,
    pub aspect_ratio: f32,
}

/// 缩放手柄尺寸
const HANDLE_SIZE: f32 = 12.0;

impl super::super::PdfReaderView {
    /// 渲染所有 PiP 图钉。
    /// 每个图钉：主体（图片+关闭按钮）+ 右下角缩放手柄。
    /// 若 image_source 为 None 则渲染空占位。
    pub(crate) fn render_pip_pins(
        &self,
        _window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.pins.is_empty() {
            return None;
        }

        let mut elements: Vec<AnyElement> = Vec::with_capacity(self.pins.len());
        for pin in &self.pins {
            let pin_id_drag = pin.id.clone();
            let pin_id_close = pin.id.clone();
            let pin_id_resize = pin.id.clone();

            let pw = pin.size.width;
            let ph = pin.size.height;
            let pin_img_src = pin.image_source.clone();

            elements.push(
                div()
                    .absolute()
                    .left(pin.position.x)
                    .top(pin.position.y)
                    // Pin 主体
                    .child(
                        div()
                            .w(pw)
                            .h(ph)
                            .rounded_lg()
                            .shadow_xl()
                            .border_1()
                            .border_color(gpui::transparent_black().opacity(0.15))
                            .bg(self.page_color_mode.bg_color())
                            .overflow_hidden()
                            .occlude()
                            .cursor_grab()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                                    let pin_id = pin_id_drag.clone();
                                    if let Some(p) = this.pins.iter().find(|p| p.id == pin_id) {
                                        this.dragging_pin = Some(PiPDragState {
                                            pin_id: pin_id.clone(),
                                            offset: Point {
                                                x: event.position.x - p.position.x,
                                                y: event.position.y - p.position.y,
                                            },
                                        });
                                        cx.notify();
                                    }
                                }),
                            )
                            .when_some(pin_img_src, |this, src| {
                                this.child(img(src).w_full().h_full())
                            })
                            // 关闭按钮（右上角）
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .right_0()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .cursor_pointer()
                                    .occlude()
                                    .child("×")
                                    .text_color(gpui::rgb(0x000000))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(
                                            move |this, _event: &MouseDownEvent, _window, cx| {
                                                this.pins.retain(|p| p.id != pin_id_close);
                                                this.dragging_pin = None;
                                                this.resizing_pin = None;
                                                cx.notify();
                                            },
                                        ),
                                    ),
                            ),
                    )
                    // 右下角缩放手柄
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .right_0()
                            .w(px(HANDLE_SIZE))
                            .h(px(HANDLE_SIZE))
                            .bg(gpui::transparent_black().opacity(0.3))
                            .rounded_tl_md()
                            .cursor_nwse_resize()
                            .occlude()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                                    let pid = pin_id_resize.clone();
                                    if let Some(p) = this.pins.iter().find(|p| p.id == pid) {
                                        let w = f32::from(p.size.width);
                                        let h = f32::from(p.size.height);
                                        this.resizing_pin = Some(PiPResizeState {
                                            pin_id: pid,
                                            start_mouse: event.position,
                                            start_bounds: Bounds {
                                                origin: Point {
                                                    x: p.position.x.into(),
                                                    y: p.position.y.into(),
                                                },
                                                size: Size {
                                                    width: w,
                                                    height: h,
                                                },
                                            },
                                            aspect_ratio: w / h,
                                        });
                                        cx.notify();
                                    }
                                }),
                            ),
                    )
                    .into_any_element(),
            );
        }

        Some(
            div()
                .absolute()
                .inset_0()
                .children(elements)
                .into_any_element(),
        )
    }

    /// 为所有 Pin 更新尺寸并发送渲染请求（缩放/底色变化后调用）
    pub(crate) fn rerender_all_pins(&mut self) {
        if self.pins.is_empty() {
            return;
        }
        let rem_size = self.last_rem_size;
        debug!(
            "pip: 重新渲染全部 {} 个 Pin (zoom={})",
            self.pins.len(),
            self.zoom_level
        );

        for pin in &mut self.pins {
            let (pdf_w, pdf_h) = self
                .page_sizes
                .get(pin.page as usize)
                .copied()
                .unwrap_or((612.0, 792.0));
            let display_w = crate::view::PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size;
            let display_h = display_w * (pdf_h / pdf_w);

            let (bx0, by0, bx1, by1) = pin.bbox;
            let bbox_w = bx1 - bx0;
            let bbox_h = by1 - by0;
            let pdf_pw = pdf_w.max(1.0);
            let new_w = bbox_w / pdf_pw * display_w;
            let new_h = bbox_h / pdf_h * display_h;

            pin.size = Size {
                width: px(new_w),
                height: px(new_h),
            };
            pin.image_source = None;

            let page = pin.page;
            let pin_id = pin.id.clone();
            let bbox = pin.bbox;
            let current_w = new_w;
            // 分辨率基于当前 CSS 尺寸，独立于 PDF zoom
            let scale = current_w * self.window_scale_factor * 1.2 / bbox_w.max(1.0);
            self.pdf_service.send_render_pin(page, pin_id, bbox, scale);
        }
    }
}
