//! 跨部件共享的纯函数工具。不依赖 `PdfReaderView` 自身状态，仅操作传入参数。
//! 用于消除 page / actions / pip 之间的代码重复。

use std::sync::Arc;

use crate::view::PAGE_BASE_WIDTH_REMS;

/// 将 muPDF 输出的 RgbaImage 包装为 GPUI 可渲染的 ImageSource。
/// 三段重复代码（cache_page_image / cache_thumbnail_image / PinRendered）统一调此函数。
pub fn make_image_source(raw: image::RgbaImage) -> gpui::ImageSource {
    let frame = image::Frame::new(raw);
    let render_img = gpui::RenderImage::new(smallvec::smallvec![frame]);
    gpui::ImageSource::Render(Arc::new(render_img))
}

/// 将 RGBA 图片写入系统剪贴板。
pub fn copy_rgba_to_clipboard(img: &image::RgbaImage) {
    use arboard::Clipboard;
    if let Ok(mut cb) = Clipboard::new() {
        cb.set_image(arboard::ImageData {
            width: img.width() as usize,
            height: img.height() as usize,
            bytes: std::borrow::Cow::from(img.as_raw().clone()),
        })
        .ok();
    }
}

/// 计算页面的显示尺寸（逻辑像素）。
/// `page_sizes` 存储 PDF 物理宽高，`page_index` 取值索引。
/// `rem_size` 为当前窗口 rem 像素值。
/// 返回 `(display_width_px, display_height_px)`。
pub fn page_display_size(
    page_sizes: &[(f32, f32)],
    page_index: usize,
    zoom_level: f32,
    rem_size: f32,
) -> (f32, f32) {
    let w = PAGE_BASE_WIDTH_REMS * zoom_level * rem_size;
    let h = page_height(page_sizes, page_index, zoom_level, rem_size);
    (w, h)
}

/// 仅计算页面的显示高度。适用于滚动条、滚动定位等只需 height 的场景。
pub fn page_height(
    page_sizes: &[(f32, f32)],
    page_index: usize,
    zoom_level: f32,
    rem_size: f32,
) -> f32 {
    let (pdf_w, pdf_h) = page_sizes
        .get(page_index)
        .copied()
        .unwrap_or((612.0, 792.0));
    (PAGE_BASE_WIDTH_REMS * zoom_level * rem_size) * (pdf_h / pdf_w)
}
