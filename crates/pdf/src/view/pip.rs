use crate::view::PdfReaderView;
use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Context, ImageSource, InteractiveElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Point, RenderImage, Size, Styled, Window, div, img, px,
};
use std::sync::Arc;

/// 画中画图钉
#[allow(dead_code)]
pub struct PiPPin {
    pub id: String,
    pub page: u16,
    pub source_page: u16,
    pub source_offset_y: f32,
    pub position: Point<Pixels>,
    pub size: Size<Pixels>,
    pub image_source: ImageSource,
}

/// 拖拽状态
#[derive(Clone)]
pub(crate) struct PiPDragState {
    pub pin_id: String,
    pub offset: Point<Pixels>,
}

/// 缩放状态
#[derive(Clone)]
pub(crate) struct PiPResizeState {
    pub pin_id: String,
    pub start_mouse: Point<Pixels>,
    pub start_bounds: Bounds<f32>,
    pub aspect_ratio: f32,
}

/// 从原始页面图像裁剪区域，生成 ImageSource（缩放转换版）
pub(crate) fn crop_and_make_source(
    raw: &image::RgbaImage,
    bounds: &Bounds<f32>,
    display_w: f32,
    display_h: f32,
) -> Option<(ImageSource, f32)> {
    let scale_x = raw.width() as f32 / display_w;
    let scale_y = raw.height() as f32 / display_h;
    let cx = (bounds.origin.x * scale_x).max(0.0) as u32;
    let cy = (bounds.origin.y * scale_y).max(0.0) as u32;
    let cw = (bounds.size.width * scale_x).max(10.0) as u32;
    let ch = (bounds.size.height * scale_y).max(10.0) as u32;
    let cw = cw.min(raw.width().saturating_sub(cx));
    let ch = ch.min(raw.height().saturating_sub(cy));
    if cw < 1 || ch < 1 {
        return None;
    }
    let cropped = image::imageops::crop_imm(raw, cx, cy, cw, ch).to_image();
    let frame = image::Frame::new(cropped);
    let render_image = RenderImage::new(vec![frame]);
    let source = ImageSource::Render(Arc::new(render_image));
    Some((source, cw as f32 / ch as f32))
}

impl PdfReaderView {
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
            let pin_img = pin.image_source.clone();

            elements.push(
                div()
                    .absolute()
                    .left(pin.position.x)
                    .top(pin.position.y)
                    .w(pin.size.width)
                    .h(pin.size.height)
                    .rounded_lg()
                    .shadow_xl()
                    .border_1()
                    .border_color(gpui::transparent_black().opacity(0.15))
                    .overflow_hidden()
                    .occlude()
                    .cursor_grab()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            let pin_id = pin_id_drag.clone();
                            if let Some(p) = this.pins.iter().find(|p| p.id == pin_id) {
                                let w = f32::from(p.size.width);
                                let h = f32::from(p.size.height);
                                let local_x = f32::from(event.position.x - p.position.x);
                                let local_y = f32::from(event.position.y - p.position.y);
                                let in_resize_zone = local_x > w - 20.0 && local_y > h - 20.0;
                                if in_resize_zone {
                                    let aspect = w / h;
                                    this.resizing_pin = Some(PiPResizeState {
                                        pin_id: pin_id.clone(),
                                        start_mouse: event.position,
                                        start_bounds: Bounds {
                                            origin: Point {
                                                x: p.position.x.into(),
                                                y: p.position.y.into(),
                                            },
                                            size: Size {
                                                width: p.size.width.into(),
                                                height: p.size.height.into(),
                                            },
                                        },
                                        aspect_ratio: aspect,
                                    });
                                } else {
                                    this.dragging_pin = Some(PiPDragState {
                                        pin_id: pin_id.clone(),
                                        offset: Point {
                                            x: event.position.x - p.position.x,
                                            y: event.position.y - p.position.y,
                                        },
                                    });
                                }
                                cx.notify();
                            }
                        }),
                    )
                    .child(img(pin_img).w_full().h_full())
                    // 关闭按钮（右上角叠加）
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                                    this.pins.retain(|p| p.id != pin_id_close);
                                    this.dragging_pin = None;
                                    this.resizing_pin = None;
                                    cx.notify();
                                }),
                            ),
                    )
                    // 缩放手柄（右下角 — 纯视觉装饰）
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .right_0()
                            .w(px(14.0))
                            .h(px(14.0))
                            .bg(gpui::black().opacity(0.25))
                            .rounded_tl_md(),
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
}
