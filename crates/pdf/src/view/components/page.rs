use super::super::PdfReaderView;
use crate::LinkPageData;
use crate::TextPageData;
use crate::view::PAGE_BASE_WIDTH_REMS;
use crate::view::types::WorkerState;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, ImageSource, InteractiveElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement, RenderImage, ScrollWheelEvent, Styled, Window,
    div, img, px,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use std::sync::Arc;

const HIGHLIGHT_TOP_OFFSET_PX: f32 = 0.5;
const HIGHLIGHT_BOTTOM_OFFSET_PX: f32 = -1.0;

fn multiply_blend_rect(
    image: &mut image::RgbaImage,
    left: f32, top: f32, right: f32, bottom: f32,
    color_rgb: (u8, u8, u8), alpha: u8,
) {
    let t = alpha as f32 / 255.0;
    let (r, g, b) = color_rgb;
    let (fr, fg, fb) = (r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let x0 = (left.max(0.0)) as u32;
    let y0 = (top.max(0.0)) as u32;
    let x1 = (right.ceil() as u32).min(image.width());
    let y1 = (bottom.ceil() as u32).min(image.height());
    for y in y0..y1 {
        for x in x0..x1 {
            let p = image.get_pixel_mut(x, y);
            p[0] = (p[0] as f32 * (fr * t + 1.0 - t)) as u8;
            p[1] = (p[1] as f32 * (fg * t + 1.0 - t)) as u8;
            p[2] = (p[2] as f32 * (fb * t + 1.0 - t)) as u8;
        }
    }
}

fn annotation_color_to_rgb(color: crate::AnnotationColor) -> (u8, u8, u8) {
    match color {
        crate::AnnotationColor::Yellow => (0xFF, 0xD4, 0x00),
        crate::AnnotationColor::Red    => (0xFF, 0x66, 0x66),
        crate::AnnotationColor::Green  => (0x5F, 0xB2, 0x36),
        crate::AnnotationColor::Blue   => (0x2E, 0xA8, 0xE5),
        crate::AnnotationColor::Purple => (0xA2, 0x8A, 0xE5),
        crate::AnnotationColor::Magenta => (0xE5, 0x6E, 0xEE),
        crate::AnnotationColor::Orange => (0xF1, 0x98, 0x37),
        crate::AnnotationColor::Gray   => (0xAA, 0xAA, 0xAA),
    }
}

fn compose_annotations(
    mut image: image::RgbaImage,
    anns: &[crate::Annotation],
    text_data: &crate::TextPageData,
    page_index: u16,
) -> image::RgbaImage {
    let scale = image.width() as f32 / text_data.display_w;
    for ann in anns {
        match &ann.kind {
            crate::AnnotationKind::Highlight | crate::AnnotationKind::Underline => {
                if let Some(ref range) = ann.range {
                    if page_index < range.start_page || page_index > range.end_page_or() {
                        continue;
                    }
                    let start = if page_index == range.start_page {
                        range.start_char
                    } else {
                        0
                    };
                    let end = if page_index == range.end_page_or() {
                        range.end_char
                    } else {
                        text_data.chars.len().saturating_sub(1)
                    };
                    if start > end || end >= text_data.chars.len() {
                        continue;
                    }
                    let blocks = text_data.merge_char_blocks(start, end);
                    let rgb = annotation_color_to_rgb(ann.color);
                    let alpha = match &ann.kind {
                        crate::AnnotationKind::Highlight => 0x80,
                        crate::AnnotationKind::Underline => 0xFF,
                        _ => unreachable!(),
                    };
                    for &(bx, by, b_max_x, b_max_y) in &blocks {
                        multiply_blend_rect(
                            &mut image,
                            bx * scale, by * scale,
                            b_max_x * scale, b_max_y * scale,
                            rgb, alpha,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    image
}

impl PdfReaderView {
    pub(crate) fn on_page_rendered(
        &mut self,
        page: u16,
        generation: u64,
        image: image::RgbaImage,
        cx: &mut Context<Self>,
    ) {
        if generation != self.render_generation {
            return;
        }
        self.raw_page_cache.put(page, Arc::new(image.clone()));
        let final_image = if let Some(text_data) = self.text_cache.peek(&page) {
            let anns = self.collect_annotations_for_page(page);
            if !anns.is_empty() {
                compose_annotations(image, &anns, text_data, page)
            } else {
                image
            }
        } else {
            image
        };
        let frame = image::Frame::new(final_image);
        let render_image = RenderImage::new(vec![frame]);
        self.page_cache
            .put(page, ImageSource::Render(Arc::new(render_image)));
        self.stale_cache.pop(&page);
        cx.notify();
    }

    pub(crate) fn on_thumbnail_rendered(
        &mut self,
        page: u16,
        image: image::RgbaImage,
        cx: &mut Context<Self>,
    ) {
        let frame = image::Frame::new(image);
        let render_image = RenderImage::new(vec![frame]);
        self.thumbnail_cache
            .put(page, ImageSource::Render(Arc::new(render_image)));
        cx.notify();
    }

    pub(crate) fn on_text_extracted(
        &mut self,
        page: u16,
        generation: u64,
        data: TextPageData,
        cx: &mut Context<Self>,
    ) {
        if generation != self.render_generation {
            return;
        }
        self.text_cache.put(page, data);

        // 写入 search_text_storage 并触发增量搜索
        if let Some(ref mut storage) = self.search_text_storage {
            if (page as usize) < storage.len() {
                storage[page as usize] = Some(self.text_cache.get(&page).unwrap().clone());
            }

            if let Some(ref state) = self.search_state
                && !state.query.is_empty()
            {
                let query_lower = state.query.to_lowercase();
                let query_chars: Vec<char> = query_lower.chars().collect();
                if !query_chars.is_empty() {
                    let new_matches = self.search_page_single(page, &query_chars);
                    self.merge_search_results(page, new_matches, cx);
                }
            }
        }

        if let (Some(raw), Some(td)) = (
            self.raw_page_cache.peek(&page),
            self.text_cache.peek(&page),
        ) {
            let anns = self.collect_annotations_for_page(page);
            if !anns.is_empty() {
                let composited = compose_annotations((**raw).clone(), &anns, td, page);
                let frame = image::Frame::new(composited);
                let render_image = Arc::new(RenderImage::new(vec![frame]));
                self.page_cache.put(page, ImageSource::Render(render_image));
            }
        }

        cx.notify();
    }

    pub(crate) fn render_list_item(
        &mut self,
        index: usize,
        scale_factor: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page_index = index as u16;
        let zoom = self.zoom_level;
        let rem_size = window.rem_size();

        // 1. 基础逻辑尺寸 (Logical Pixels)
        let display_width_px = PAGE_BASE_WIDTH_REMS * zoom * f32::from(rem_size);
        let (pdf_w, pdf_h) = self
            .page_sizes
            .get(index)
            .copied()
            .unwrap_or((612.0, 792.0));
        let display_height_px = display_width_px * (pdf_h / pdf_w);

        self.load_page_text_with_size(page_index, display_width_px, display_height_px, cx);
        self.load_page_links_with_size(page_index, display_width_px, display_height_px, cx);

        if self.annotation_version != self.last_composited_version {
            let dirty: Vec<u16> = self.raw_page_cache.iter().map(|(k, _)| *k).collect();
            for &p in &dirty {
                let anns = self.collect_annotations_for_page(p);
                if anns.is_empty() { continue; }
                if let (Some(raw), Some(td)) = (
                    self.raw_page_cache.peek(&p).cloned(),
                    self.text_cache.peek(&p).cloned(),
                ) {
                    let img = compose_annotations((*raw).clone(), &anns, &td, p);
                    let frame = image::Frame::new(img);
                    let ri = Arc::new(RenderImage::new(vec![frame]));
                    self.page_cache.put(p, ImageSource::Render(ri));
                }
            }
            self.last_composited_version = self.annotation_version;
        }

        let content = {
            self.load_page_to_cache(page_index, scale_factor, cx);
            if self.page_cache.contains(&page_index) {
                let img_src = self.page_cache.get(&page_index).unwrap();
                img(img_src.clone())
                    .w(px(display_width_px))
                    .h(px(display_height_px))
                    .into_any_element()
            } else if self.stale_cache.contains(&page_index) {
                let img_src = self.stale_cache.get(&page_index).unwrap();
                img(img_src.clone())
                    .w(px(display_width_px))
                    .h(px(display_height_px))
                    .into_any_element()
            } else {
                div()
                    .w(px(display_width_px))
                    .h(px(display_height_px))
                    .bg(cx.theme().background)
                    .child(
                        h_flex()
                            .size_full()
                            .items_center()
                            .justify_center()
                            .child("Loading..."),
                    )
                    .into_any_element()
            }
        };

        // 选区层
        let selection_highlight = self.render_selection_highlight(page_index, window, cx);

        // 注释层
        let annotation_overlay = self.render_annotation_overlay(page_index, window, cx);

        // 链接层
        let link_overlay = self.render_link_overlay(page_index, window, cx);

        // 组合渲染：建立坐标沙盒
        v_flex()
            .w_full()
            .items_center()
            .child(
                div()
                    .shadow_lg()
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(gpui::white())
                    .left(px(self.offset_x))
                    .w(px(display_width_px))
                    .h(px(display_height_px))
                    .child(
                        div()
                            .relative()
                            .overflow_hidden()
                            .size_full()
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                    if let Some((pid, px_x, px_y)) = this.content_to_page_coords(
                                        event.position.x,
                                        event.position.y,
                                        window,
                                    ) && pid == page_index
                                        && let Some(ann_id) = this.hit_test_annotation(
                                            pid,
                                            f32::from(px_x),
                                            f32::from(px_y),
                                            window,
                                        )
                                    {
                                        this.annotation_state.selected_id = Some(ann_id.clone());
                                        this.annotation_state.context_menu =
                                            Some(crate::ContextMenuState {
                                                annotation_id: ann_id,
                                                position: event.position,
                                                from_sidebar: false,
                                            });
                                        cx.notify();
                                        return;
                                    }
                                    // 未命中任何注释 → 清除
                                    this.annotation_state.selected_id = None;
                                    this.annotation_state.context_menu = None;
                                    cx.notify();
                                }),
                            )
                            .child(content)
                            .when_some(selection_highlight, |this, hl| this.child(hl))
                            .when_some(annotation_overlay, |this, hl| this.child(hl))
                            .when_some(link_overlay, |this, hl| this.child(hl))
                            .when_some(
                                self.render_search_highlight(page_index, window, cx),
                                |this, hl| this.child(hl),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn render_selection_highlight(
        &mut self,
        page_index: u16,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let mut sp = self.selection_start?;
        let mut ep = self.selection_end?;
        if sp.0 > ep.0 || (sp.0 == ep.0 && sp.1 > ep.1) {
            std::mem::swap(&mut sp, &mut ep);
        }

        if page_index < sp.0 || page_index > ep.0 {
            return None;
        }

        let text_data = self.text_cache.get(&page_index)?;
        let start = if page_index == sp.0 { sp.1 } else { 0 };
        let end = if page_index == ep.0 {
            ep.1
        } else {
            text_data.chars.len().saturating_sub(1)
        };

        let blocks = text_data.merge_char_blocks(start, end);
        if blocks.is_empty() {
            return None;
        }

        let highlights: Vec<_> = blocks
            .iter()
            .map(|&(bx, by, b_max_x, b_max_y)| {
                div()
                    .absolute()
                    .left(px(bx))
                    .top(px(by + HIGHLIGHT_TOP_OFFSET_PX))
                    .w(px(b_max_x - bx))
                    .h(px((b_max_y - by + HIGHLIGHT_BOTTOM_OFFSET_PX).max(1.0)))
                    .bg(gpui::rgba(0x4285f470))
                    .rounded(px(2.0))
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .absolute()
                .inset_0()
                .children(highlights)
                .into_any_element(),
        )
    }

    pub(crate) fn render_search_highlight(
        &mut self,
        page_index: u16,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let m = self
            .search_state
            .as_ref()
            .and_then(|s| s.active_match())
            .cloned()?;
        if m.page_index != page_index {
            return None;
        }

        let text_data = self
            .search_text_storage
            .as_ref()
            .and_then(|s| s.get(page_index as usize).and_then(|d| d.as_ref()))?;
        let blocks = text_data.merge_char_blocks(m.start_char, m.end_char);
        if blocks.is_empty() {
            return None;
        }

        let highlights: Vec<_> = blocks
            .iter()
            .map(|&(bx, by, b_max_x, b_max_y)| {
                div()
                    .absolute()
                    .left(px(bx))
                    .top(px(by + HIGHLIGHT_TOP_OFFSET_PX))
                    .w(px(b_max_x - bx))
                    .h(px((b_max_y - by + HIGHLIGHT_BOTTOM_OFFSET_PX).max(1.0)))
                    .bg(gpui::rgba(0xf59e0b70))
                    .rounded(px(2.0))
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .absolute()
                .inset_0()
                .children(highlights)
                .into_any_element(),
        )
    }

    fn collect_annotations_for_page(&self, page_index: u16) -> Vec<crate::Annotation> {
        let mut result = Vec::new();
        if let Some(anns) = self.annotation_state.annotations.get(&page_index) {
            for ann in anns {
                if !ann.is_deleted {
                    result.push(ann.clone());
                }
            }
        }
        for (_, anns) in &self.annotation_state.annotations {
            for ann in anns {
                if !ann.is_deleted
                    && let Some(ref range) = ann.range
                    && range.start_page < page_index
                    && range.end_page_or() >= page_index
                {
                    result.push(ann.clone());
                }
            }
        }
        result
    }

    fn get_annotation_gpui_color(
        color: crate::AnnotationColor,
        kind: &crate::AnnotationKind,
    ) -> gpui::Hsla {
        let alpha: u32 = match kind {
            crate::AnnotationKind::Highlight => 0x80,
            crate::AnnotationKind::Underline => 0xFF,
            crate::AnnotationKind::Rectangle { .. } => 0x60,
        };
        let rgb = match color {
            crate::AnnotationColor::Yellow => 0xFFD400,
            crate::AnnotationColor::Red => 0xFF6666,
            crate::AnnotationColor::Green => 0x5FB236,
            crate::AnnotationColor::Blue => 0x2EA8E5,
            crate::AnnotationColor::Purple => 0xA28AE5,
            crate::AnnotationColor::Magenta => 0xE56EEE,
            crate::AnnotationColor::Orange => 0xF19837,
            crate::AnnotationColor::Gray => 0xAAAAAA,
        };
        gpui::rgba((rgb << 8) | alpha).into()
    }

    fn create_annotation_element(
        bx: f32,
        _by: f32,
        b_max_x: f32,
        b_max_y: f32,
        color: gpui::Hsla,
        kind: &crate::AnnotationKind,
    ) -> AnyElement {
        match kind {
            crate::AnnotationKind::Highlight => div().into_any_element(),
            crate::AnnotationKind::Underline => div()
                .absolute()
                .left(px(bx))
                .top(px(b_max_y - 2.0))
                .w(px((b_max_x - bx).max(1.0)))
                .h(px(2.0))
                .bg(color)
                .into_any_element(),
            crate::AnnotationKind::Rectangle { .. } => div().into_any_element(),
        }
    }

    pub(crate) fn render_annotation_overlay(
        &mut self,
        page_index: u16,
        window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let anns = self.collect_annotations_for_page(page_index);
        if anns.is_empty() && self.rect_in_progress.is_none() {
            return None;
        }

        let text_data = self.text_cache.get(&page_index);
        let display_width_px =
            PAGE_BASE_WIDTH_REMS * self.zoom_level * f32::from(window.rem_size());
        let (pdf_w, pdf_h) = self
            .page_sizes
            .get(page_index as usize)
            .copied()
            .unwrap_or((612.0, 792.0));
        let display_height_px = display_width_px * (pdf_h / pdf_w);

        let mut elements: Vec<AnyElement> = Vec::new();

        for ann in &anns {
            let color = Self::get_annotation_gpui_color(ann.color, &ann.kind);
            let is_selected = self
                .annotation_state
                .selected_id
                .as_ref()
                .is_some_and(|id| id == &ann.id);
            match &ann.kind {
                crate::AnnotationKind::Highlight | crate::AnnotationKind::Underline => {
                    if let Some(ref range) = ann.range {
                        if page_index < range.start_page || page_index > range.end_page_or() {
                            continue;
                        }
                        let start = if page_index == range.start_page {
                            range.start_char
                        } else {
                            0
                        };
                        let end = if page_index == range.end_page_or() {
                            range.end_char
                        } else if let Some(td) = text_data {
                            td.chars.len().saturating_sub(1)
                        } else {
                            continue;
                        };
                        if let Some(td) = text_data
                            && start <= end
                            && end < td.chars.len()
                        {
                            let blocks = td.merge_char_blocks(start, end);
                            let is_highlight = matches!(&ann.kind, crate::AnnotationKind::Highlight);
                            // Highlight: 颜色已混入图片，不生成填充元素
                            // Underline: 保持原有填充
                            if !is_highlight {
                                for block in &blocks {
                                    elements.push(Self::create_annotation_element(
                                        block.0, block.1, block.2, block.3, color, &ann.kind,
                                    ));
                                }
                            }
                            // 选中时在所有字符块外围画一个框（Highlight 和 Underline 都保留）
                            if is_selected {
                                if let Some((mx, my, mmx, mmy)) = blocks.iter().fold(
                                    None::<(f32, f32, f32, f32)>,
                                    |acc, &(bx, by, b_max_x, b_max_y)| {
                                        Some(match acc {
                                            Some((x0, y0, x1, y1)) => (
                                                x0.min(bx),
                                                y0.min(by),
                                                x1.max(b_max_x),
                                                y1.max(b_max_y),
                                            ),
                                            None => (bx, by, b_max_x, b_max_y),
                                        })
                                    },
                                ) {
                                    elements.push(
                                        div()
                                            .absolute()
                                            .left(px(mx - 4.0))
                                            .top(px(my - 4.0))
                                            .w(px((mmx - mx + 8.0).max(1.0)))
                                            .h(px((mmy - my + 8.0).max(1.0)))
                                            .border_3()
                                            .border_color(color)
                                            .rounded(px(2.0))
                                            .into_any_element(),
                                    );
                                }
                            }
                        }
                    }
                }
                crate::AnnotationKind::Rectangle { x, y, w, h } => {
                    let mut rect = div()
                        .absolute()
                        .left(px(x * display_width_px))
                        .top(px(y * display_height_px))
                        .w(px((w * display_width_px).max(1.0)))
                        .h(px((h * display_height_px).max(1.0)))
                        .rounded(px(2.0))
                        .border_color(color);
                    if is_selected {
                        rect = rect.border_6();
                    } else {
                        rect = rect.border_3();
                    }
                    elements.push(rect.into_any_element());
                }
            }
        }

        if let Some((pid, ref bounds)) = self.rect_in_progress {
            if pid == page_index {
                let color: gpui::Hsla = gpui::rgba(0x4285f460).into();
                let bw = bounds.right() - bounds.left();
                let bh = bounds.bottom() - bounds.top();
                elements.push(
                    div()
                        .absolute()
                        .left(px(bounds.left()))
                        .top(px(bounds.top()))
                        .w(px(bw.max(1.0)))
                        .h(px(bh.max(1.0)))
                        .border_3()
                        .border_color(color)
                        .rounded(px(2.0))
                        .into_any_element(),
                );
            }
        }

        if elements.is_empty() {
            return None;
        }

        Some(
            div()
                .absolute()
                .inset_0()
                .children(elements)
                .into_any_element(),
        )
    }

    pub(crate) fn load_page_to_cache(
        &mut self,
        page_index: u16,
        scale_factor: f32,
        _cx: &mut Context<Self>,
    ) {
        if self.page_cache.contains(&page_index) || self.worker_state != WorkerState::Running {
            return;
        }

        let scale = self.render_zoom * scale_factor * 2.0;
        let generation = self.render_generation;
        let service = self.pdf_service.clone();

        service.send_render(page_index, scale, generation);
    }

    pub(crate) fn load_page_links_with_size(
        &mut self,
        page_index: u16,
        display_width_px: f32,
        display_height_px: f32,
        _cx: &mut Context<Self>,
    ) {
        if self
            .link_cache
            .get(&page_index)
            .is_some_and(|d| d.display_w == display_width_px && d.display_h == display_height_px)
            || self.worker_state != WorkerState::Running
        {
            return;
        }

        let generation = self.render_generation;
        let service = self.pdf_service.clone();

        service.send_links(page_index, display_width_px, display_height_px, generation);
    }

    pub(crate) fn load_page_text_with_size(
        &mut self,
        page_index: u16,
        display_width_px: f32,
        display_height_px: f32,
        _cx: &mut Context<Self>,
    ) {
        // 缓存有效必须 display_w 匹配当前缩放，否则视为 miss 重新提取
        if self
            .text_cache
            .get(&page_index)
            .is_some_and(|d| d.display_w == display_width_px)
            || self.worker_state != WorkerState::Running
        {
            return;
        }

        let generation = self.render_generation;
        let service = self.pdf_service.clone();

        service.send_text(page_index, display_width_px, display_height_px, generation);
    }

    pub(crate) fn render_link_overlay(
        &mut self,
        page_index: u16,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let link_data: Option<LinkPageData> = self.link_cache.get(&page_index).cloned();
        let link_data = link_data?;

        if link_data.links.is_empty() {
            return None;
        }

        let elements: Vec<AnyElement> = link_data
            .links
            .iter()
            .map(|link| {
                div()
                    .absolute()
                    .left(px(link.left))
                    .top(px(link.top))
                    .w(px((link.right - link.left).max(1.0)))
                    .h(px((link.bottom - link.top).max(1.0)))
                    .cursor_pointer()
                    .into_any_element()
            })
            .collect();

        Some(
            div()
                .absolute()
                .inset_0()
                .children(elements)
                .into_any_element(),
        )
    }

    pub(crate) fn render_main_content(
        &mut self,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view_weak = _cx.entity().downgrade();

        div()
            .id("pdf-main-view")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(_cx.theme().muted.opacity(0.3))
            .on_mouse_down(
                MouseButton::Left,
                _cx.listener(|this, event: &MouseDownEvent, _window, _cx| {
                    if this.overlay_button_clicked {
                        this.overlay_button_clicked = false;
                        return;
                    }
                    this.is_mouse_down = true;
                    this.mouse_down_pos = Some(event.position);
                }),
            )
            .on_mouse_move(_cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.handle_content_mouse_move(event, window, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                _cx.listener(|this, _event: &MouseUpEvent, window, cx| {
                    this.handle_content_mouse_up(window, cx);
                }),
            )
            .on_scroll_wheel(_cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                if this.is_mouse_down {
                    return;
                }
                match event.delta {
                    gpui::ScrollDelta::Pixels(p) => {
                        this.apply_horizontal_scroll(f32::from(p.x), window);
                        cx.notify();
                    }
                    gpui::ScrollDelta::Lines(l) => {
                        this.apply_horizontal_scroll(l.x * 20.0, window);
                        cx.notify();
                    }
                }
            }))
            .child(
                // 页面列表
                div()
                    .size_full()
                    .child(
                        gpui::list(self.list_state.clone(), move |ix, _win, cx| {
                            let scale_factor = _win.scale_factor();
                            view_weak
                                .update(cx, |this, cx| {
                                    this.render_list_item(ix, scale_factor, _win, cx)
                                })
                                .unwrap_or_else(|_| div().into_any_element())
                        })
                        .size_full(),
                    )
                    // 滚动条占位
                    .child(self.render_scrollbar(window, _cx)),
            )
    }
}
