use gpui::prelude::*;
use gpui::{
    AnyElement, Bounds, Context, ImageSource, InteractiveElement, MouseButton, MouseDownEvent,
    ParentElement, PathPromptOptions, Pixels, Point, Size, Styled, Window, div, img, px,
};
use gpui_component::ActiveTheme;
use log::debug;
use std::sync::Arc;

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
    /// 原始 RGBA 像素缓存（供剪贴板复制用）
    pub raw_image: Option<Arc<image::RgbaImage>>,
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
            let pin_id_ctx = pin.id.clone();

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
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                                    cx.stop_propagation();
                                    this.pin_context_menu = Some((
                                        pin_id_ctx.clone(),
                                        event.position,
                                        this.pins
                                            .iter()
                                            .find(|p| p.id == pin_id_ctx)
                                            .map(|p| p.raw_image.is_some())
                                            .unwrap_or(false),
                                    ));
                                    cx.notify();
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

    /// 渲染 Pin 右键菜单（"复制为图片"）。
    pub(crate) fn render_pin_context_menu(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (pin_id, pos, has_raw) = self.pin_context_menu.as_ref()?;
        if !has_raw {
            return None;
        }
        let pin_id = pin_id.clone();
        let pin_id_save = pin_id.clone();
        let theme = cx.theme();
        let adjusted_pos = self.adjust_context_menu_position(*pos, window);
        let pos_x = f32::from(adjusted_pos.x);
        let pos_y = f32::from(adjusted_pos.y) - 20.0;
        let ctx_w = 140.0;

        Some(
            div()
                .absolute()
                .left(px(pos_x.max(0.0)))
                .top(px(pos_y.max(0.0)))
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .rounded_md()
                .p_1()
                .cursor_default()
                .min_w(px(ctx_w))
                .child(
                    div()
                        .w_full()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                        .rounded_sm()
                        .child("复制为图片")
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.pin_context_menu = None;
                                this.copy_pin_image(&pin_id);
                                cx.notify();
                            }),
                        ),
                )
                .child(
                    div()
                        .w_full()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                        .rounded_sm()
                        .child("另存为图片")
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.pin_context_menu = None;
                                this.save_pin_image(&pin_id_save, cx);
                                cx.notify();
                            }),
                        ),
                )
                .on_mouse_down(
                    gpui::MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.pin_context_menu = None;
                        cx.notify();
                    }),
                )
                .into_any_element(),
        )
    }

    /// 将指定 Pin 的原始渲染图复制到系统剪贴板。
    fn copy_pin_image(&mut self, pin_id: &str) {
        let raw = self
            .pins
            .iter()
            .find(|p| p.id == pin_id)
            .and_then(|p| p.raw_image.clone());
        if let Some(img) = raw {
            crate::view::helpers::copy_rgba_to_clipboard(&img);
        }
    }

    /// 将指定 Pin 的原始渲染图另存为 PNG 文件。
    fn save_pin_image(&mut self, pin_id: &str, cx: &mut Context<Self>) {
        let raw = self
            .pins
            .iter()
            .find(|p| p.id == pin_id)
            .and_then(|p| p.raw_image.clone());
        let Some(img) = raw else { return };
        let name = self.document_title.clone();

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择保存位置".into()),
        });

        cx.background_executor()
            .spawn(async move {
                if let Ok(Ok(Some(paths))) = receiver.await {
                    if let Some(dir) = paths.first() {
                        let path = dir.join(format!("{}.png", name));
                        let _ = img.save(&path);
                    }
                }
            })
            .detach();
    }
}
