use super::{
    AnnotationResizeHandle, PAGE_BASE_WIDTH_REMS, PdfReaderView, SIDEBAR_MAX_RATIO,
    SIDEBAR_MIN_RATIO, TOOLBAR_HEIGHT_REMS, TranslationResult, helpers,
};
use chrono::Utc;
use gpui::{Context, MouseMoveEvent, Pixels, Point, Size, Window, px};
use log::debug;
use uuid::Uuid;

impl PdfReaderView {
    pub(crate) fn handle_root_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_dragging_scrollbar {
            self.scroll_to_position(
                event.position.y,
                window.viewport_size().height,
                window.rem_size(),
                cx,
            );
            return;
        }

        if self.is_dragging_thumbnail_scrollbar {
            let view_height_px = f32::from(window.viewport_size().height);
            let sidebar_content_height_px = view_height_px - 36.0;
            self.scroll_thumbnails_to_position(event.position.y, sidebar_content_height_px, cx);
            return;
        }

        if self.dragging_left_resizer {
            let viewport_width = window.viewport_size().width;
            let min_width = px(f32::from(viewport_width) * SIDEBAR_MIN_RATIO);
            let max_width = px(f32::from(viewport_width) * SIDEBAR_MAX_RATIO);
            let current_right_w = if self.is_right_sidebar_open {
                self.right_sidebar_width
            } else {
                px(0.0)
            };
            let available_for_left = (viewport_width - current_right_w - px(300.0)).max(min_width);
            let final_max = max_width.min(available_for_left);

            self.left_sidebar_width = event.position.x.max(min_width).min(final_max);
            self.preferred_left_sidebar_width = f32::from(self.left_sidebar_width);
            cx.notify();
            return;
        }

        if self.dragging_right_resizer {
            let viewport_width = window.viewport_size().width;
            let min_width = px(f32::from(viewport_width) * SIDEBAR_MIN_RATIO);
            let max_width = px(f32::from(viewport_width) * SIDEBAR_MAX_RATIO);
            let current_left_w = if self.is_left_sidebar_open {
                self.left_sidebar_width
            } else {
                px(0.0)
            };
            let available_for_right = (viewport_width - current_left_w - px(300.0)).max(min_width);
            let final_max = max_width.min(available_for_right);

            self.right_sidebar_width = (viewport_width - event.position.x)
                .max(min_width)
                .min(final_max);
            self.preferred_right_sidebar_width = f32::from(self.right_sidebar_width);
            cx.notify();
        }
    }

    pub(crate) fn handle_pin_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref resize) = self.resizing_pin.clone() {
            if let Some(pin) = self.pins.iter_mut().find(|p| p.id == resize.pin_id) {
                let dx = f32::from(event.position.x - resize.start_mouse.x);
                let mut new_w = (resize.start_bounds.size.width + dx).max(100.0);
                let mut new_h = new_w / resize.aspect_ratio;

                let rem_size = window.rem_size();
                let toolbar_h = f32::from(gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(rem_size));
                let tab_bar_h = self.tab_bar_offset_rems * f32::from(rem_size);
                let mut max_w = f32::from(window.viewport_size().width) * 0.9;
                if self.is_left_sidebar_open {
                    max_w -= f32::from(self.left_sidebar_width);
                }
                if self.is_right_sidebar_open {
                    max_w -= f32::from(self.right_sidebar_width);
                }
                let max_h =
                    (f32::from(window.viewport_size().height) - tab_bar_h - toolbar_h) * 0.9;

                if new_w > max_w {
                    new_w = max_w;
                    new_h = new_w / resize.aspect_ratio;
                }
                if new_h > max_h {
                    new_h = max_h;
                    new_w = new_h * resize.aspect_ratio;
                }

                pin.size = Size {
                    width: px(new_w),
                    height: px(new_h),
                };
                pin.image_source = None;
                let page = pin.page;
                let pin_id = pin.id.clone();
                let bbox = pin.bbox;
                let current_w = new_w;
                cx.notify();

                // 发送独立于 PDF zoom 的渲染请求
                let bbox_w = (bbox.2 - bbox.0).max(1.0);
                let scale = current_w * self.window_scale_factor * 1.2 / bbox_w;
                self.pdf_service.send_render_pin(page, pin_id, bbox, scale);
            }
        } else if let Some(ref drag) = self.dragging_pin.clone() {
            if let Some(pin) = self.pins.iter_mut().find(|p| p.id == drag.pin_id) {
                let rem_size = window.rem_size();
                let toolbar_h = f32::from(gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(rem_size));
                let tab_bar_h = self.tab_bar_offset_rems * f32::from(rem_size);
                let max_x = f32::from(window.viewport_size().width) - f32::from(pin.size.width);
                let max_y = f32::from(window.viewport_size().height)
                    - tab_bar_h
                    - toolbar_h
                    - f32::from(pin.size.height);
                pin.position = Point {
                    x: (event.position.x - drag.offset.x).clamp(px(0.0), px(max_x.max(0.0))),
                    y: (event.position.y - drag.offset.y).clamp(px(0.0), px(max_y.max(0.0))),
                };
                cx.notify();
            }
        } else if let Some(ref drag) = self.annotation_drag.clone() {
            let page_index = drag.page;
            let rem_size = f32::from(window.rem_size());
            let (display_w, display_h) = helpers::page_display_size(
                &self.page_sizes,
                page_index as usize,
                self.zoom_level,
                rem_size,
            );

            let dx_px = f32::from(event.position.x - drag.start_mouse.x);
            let dy_px = f32::from(event.position.y - drag.start_mouse.y);

            let dx = dx_px / display_w;
            let dy = dy_px / display_h;

            let mut new_x = drag.start_x;
            let mut new_y = drag.start_y;
            let mut new_w = drag.start_w;
            let mut new_h = drag.start_h;

            match drag.handle {
                AnnotationResizeHandle::TopLeft => {
                    new_x = (drag.start_x + dx).clamp(0.0, drag.start_x + drag.start_w - 0.01);
                    new_y = (drag.start_y + dy).clamp(0.0, drag.start_y + drag.start_h - 0.01);
                    new_w = drag.start_w - (new_x - drag.start_x);
                    new_h = drag.start_h - (new_y - drag.start_y);
                }
                AnnotationResizeHandle::Top => {
                    new_y = (drag.start_y + dy).clamp(0.0, drag.start_y + drag.start_h - 0.01);
                    new_h = drag.start_h - (new_y - drag.start_y);
                }
                AnnotationResizeHandle::TopRight => {
                    new_y = (drag.start_y + dy).clamp(0.0, drag.start_y + drag.start_h - 0.01);
                    new_h = drag.start_h - (new_y - drag.start_y);
                    new_w = (drag.start_w + dx).clamp(0.01, 1.0 - new_x);
                }
                AnnotationResizeHandle::Right => {
                    new_w = (drag.start_w + dx).clamp(0.01, 1.0 - new_x);
                }
                AnnotationResizeHandle::BottomRight => {
                    new_w = (drag.start_w + dx).clamp(0.01, 1.0 - new_x);
                    new_h = (drag.start_h + dy).clamp(0.01, 1.0 - new_y);
                }
                AnnotationResizeHandle::Bottom => {
                    new_h = (drag.start_h + dy).clamp(0.01, 1.0 - new_y);
                }
                AnnotationResizeHandle::BottomLeft => {
                    new_x = (drag.start_x + dx).clamp(0.0, drag.start_x + drag.start_w - 0.01);
                    new_w = drag.start_w - (new_x - drag.start_x);
                    new_h = (drag.start_h + dy).clamp(0.01, 1.0 - new_y);
                }
                AnnotationResizeHandle::Left => {
                    new_x = (drag.start_x + dx).clamp(0.0, drag.start_x + drag.start_w - 0.01);
                    new_w = drag.start_w - (new_x - drag.start_x);
                }
                AnnotationResizeHandle::TextStart | AnnotationResizeHandle::TextEnd => {}
            }

            if let Some(anns) = self.annotation_state.annotations.get_mut(&page_index)
                && let Some(ann) = anns.iter_mut().find(|a| a.id == drag.annotation_id)
            {
                if !matches!(
                    drag.handle,
                    AnnotationResizeHandle::TextStart | AnnotationResizeHandle::TextEnd
                ) {
                    ann.kind = crate::AnnotationKind::Rectangle {
                        x: new_x,
                        y: new_y,
                        w: new_w,
                        h: new_h,
                    };
                }
                ann.is_dirty = true;
                self.annotation_version += 1;
                cx.notify();
            }
        }
    }

    pub(crate) fn handle_content_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref drag) = self.annotation_drag.clone() {
            let page_index = drag.page;
            let rem_size = f32::from(window.rem_size());
            let (display_w, display_h) = helpers::page_display_size(
                &self.page_sizes,
                page_index as usize,
                self.zoom_level,
                rem_size,
            );

            let dx_px = f32::from(event.position.x - drag.start_mouse.x);
            let dy_px = f32::from(event.position.y - drag.start_mouse.y);

            let dx = dx_px / display_w;
            let dy = dy_px / display_h;

            let mut new_x = drag.start_x;
            let mut new_y = drag.start_y;
            let mut new_w = drag.start_w;
            let mut new_h = drag.start_h;

            let mut char_idx_opt = None;
            if matches!(
                drag.handle,
                AnnotationResizeHandle::TextStart | AnnotationResizeHandle::TextEnd
            ) && let Some((pid, local_x, local_y)) =
                self.content_to_page_coords(event.position.x, event.position.y, window)
                && pid == page_index
            {
                char_idx_opt = self.find_char_at_position(page_index, local_x, local_y, window);
            }

            match drag.handle {
                AnnotationResizeHandle::TopLeft => {
                    new_x = (drag.start_x + dx).clamp(0.0, drag.start_x + drag.start_w - 0.01);
                    new_y = (drag.start_y + dy).clamp(0.0, drag.start_y + drag.start_h - 0.01);
                    new_w = drag.start_w - (new_x - drag.start_x);
                    new_h = drag.start_h - (new_y - drag.start_y);
                }
                AnnotationResizeHandle::Top => {
                    new_y = (drag.start_y + dy).clamp(0.0, drag.start_y + drag.start_h - 0.01);
                    new_h = drag.start_h - (new_y - drag.start_y);
                }
                AnnotationResizeHandle::TopRight => {
                    new_y = (drag.start_y + dy).clamp(0.0, drag.start_y + drag.start_h - 0.01);
                    new_h = drag.start_h - (new_y - drag.start_y);
                    new_w = (drag.start_w + dx).clamp(0.01, 1.0 - new_x);
                }
                AnnotationResizeHandle::Right => {
                    new_w = (drag.start_w + dx).clamp(0.01, 1.0 - new_x);
                }
                AnnotationResizeHandle::BottomRight => {
                    new_w = (drag.start_w + dx).clamp(0.01, 1.0 - new_x);
                    new_h = (drag.start_h + dy).clamp(0.01, 1.0 - new_y);
                }
                AnnotationResizeHandle::Bottom => {
                    new_h = (drag.start_h + dy).clamp(0.01, 1.0 - new_y);
                }
                AnnotationResizeHandle::BottomLeft => {
                    new_x = (drag.start_x + dx).clamp(0.0, drag.start_x + drag.start_w - 0.01);
                    new_w = drag.start_w - (new_x - drag.start_x);
                    new_h = (drag.start_h + dy).clamp(0.01, 1.0 - new_y);
                }
                AnnotationResizeHandle::Left => {
                    new_x = (drag.start_x + dx).clamp(0.0, drag.start_x + drag.start_w - 0.01);
                    new_w = drag.start_w - (new_x - drag.start_x);
                }
                _ => {}
            }

            if let Some(anns) = self.annotation_state.annotations.get_mut(&page_index)
                && let Some(ann) = anns.iter_mut().find(|a| a.id == drag.annotation_id)
            {
                match drag.handle {
                    AnnotationResizeHandle::TextStart => {
                        if let Some(char_idx) = char_idx_opt
                            && let Some(ref mut range) = ann.range
                            && char_idx <= range.end_char
                        {
                            range.start_char = char_idx;
                            ann.is_dirty = true;
                            self.annotation_version += 1;
                            cx.notify();
                        }
                    }
                    AnnotationResizeHandle::TextEnd => {
                        if let Some(char_idx) = char_idx_opt
                            && let Some(ref mut range) = ann.range
                            && char_idx >= range.start_char
                        {
                            range.end_char = char_idx;
                            ann.is_dirty = true;
                            self.annotation_version += 1;
                            cx.notify();
                        }
                    }
                    _ => {
                        ann.kind = crate::AnnotationKind::Rectangle {
                            x: new_x,
                            y: new_y,
                            w: new_w,
                            h: new_h,
                        };
                        ann.is_dirty = true;
                        self.annotation_version += 1;
                        cx.notify();
                    }
                }
            }
            return;
        }

        if self.is_mouse_down {
            if !self.is_selecting
                && !self.is_panning
                && let Some(down_pos) = self.mouse_down_pos
            {
                let dx = f32::from(event.position.x) - f32::from(down_pos.x);
                let dy = f32::from(event.position.y) - f32::from(down_pos.y);
                if dx.powi(2) + dy.powi(2) > 25.0 {
                    if matches!(
                        self.annotation_state.active_tool,
                        crate::AnnotationTool::Rectangle(_)
                    ) || self.annotation_state.active_tool == crate::AnnotationTool::Pin
                    {
                        self.is_selecting = true;
                    } else if let Some((page_index, local_x, local_y)) =
                        self.content_to_page_coords(down_pos.x, down_pos.y, window)
                    {
                        if let Some(char_idx) =
                            self.find_char_at_position(page_index, local_x, local_y, window)
                        {
                            debug!(
                                "SELECT: start_selection page={}, char={} at local=({:.1},{:.1})",
                                page_index,
                                char_idx,
                                f32::from(local_x),
                                f32::from(local_y)
                            );
                            self.is_selecting = true;
                            self.start_selection(page_index, char_idx, cx);
                        } else {
                            self.is_panning = true;
                        }
                    }
                }
            }

            if self.is_panning
                && let Some(down_pos) = self.mouse_down_pos
            {
                let dx = f32::from(event.position.x) - f32::from(down_pos.x);
                let dy = f32::from(event.position.y) - f32::from(down_pos.y);

                // 横向平移并限制边界
                let rem_size_px = f32::from(window.rem_size());
                let display_width_px = PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size_px;
                let mut available_width = f32::from(window.viewport_size().width);
                if self.is_left_sidebar_open {
                    available_width -= f32::from(self.left_sidebar_width);
                }
                if self.is_right_sidebar_open {
                    available_width -= f32::from(self.right_sidebar_width);
                }

                if !self.fit_to_width_mode && display_width_px > available_width {
                    let max_offset = (display_width_px - available_width) / 2.0;
                    self.offset_x = (self.offset_x + dx).clamp(-max_offset, max_offset);
                } else {
                    self.offset_x = 0.0;
                }

                // 纵向平移 (支持跨页与边界限制)
                let scroll_top = self.list_state.logical_scroll_top();
                let mut target_offset = f32::from(scroll_top.offset_in_item) - dy;
                let mut target_ix = scroll_top.item_ix;

                while target_offset < 0.0 && target_ix > 0 {
                    target_ix -= 1;
                    let page_h = helpers::page_height(
                        &self.page_sizes,
                        target_ix,
                        self.zoom_level,
                        rem_size_px,
                    );
                    target_offset += page_h;
                }
                if target_ix == 0 {
                    target_offset = target_offset.max(0.0);
                }

                while target_ix < self.total_pages.saturating_sub(1) {
                    let page_h = helpers::page_height(
                        &self.page_sizes,
                        target_ix,
                        self.zoom_level,
                        rem_size_px,
                    );
                    if target_offset > page_h {
                        target_offset -= page_h;
                        target_ix += 1;
                    } else {
                        break;
                    }
                }
                if target_ix == self.total_pages.saturating_sub(1) {
                    let page_h = helpers::page_height(
                        &self.page_sizes,
                        target_ix,
                        self.zoom_level,
                        rem_size_px,
                    );
                    target_offset = target_offset.min(page_h);
                }

                self.list_state.scroll_to(gpui::ListOffset {
                    item_ix: target_ix,
                    offset_in_item: px(target_offset),
                });

                self.mouse_down_pos = Some(event.position);
                cx.notify();
                return;
            }

            if self.is_selecting {
                if matches!(
                    self.annotation_state.active_tool,
                    crate::AnnotationTool::Rectangle(_)
                ) || self.annotation_state.active_tool == crate::AnnotationTool::Pin
                {
                    if let Some(down_pos) = self.mouse_down_pos
                        && let Some((start_page, start_x, start_y)) =
                            self.content_to_page_coords(down_pos.x, down_pos.y, window)
                        && let Some((curr_page, curr_x, curr_y)) =
                            self.content_to_page_coords(event.position.x, event.position.y, window)
                        && start_page == curr_page
                    {
                        let x = start_x.min(curr_x);
                        let y = start_y.min(curr_y);
                        let w = (start_x - curr_x).abs();
                        let h = (start_y - curr_y).abs();
                        self.rect_in_progress = Some((
                            start_page,
                            gpui::Bounds {
                                origin: gpui::Point {
                                    x: f32::from(x),
                                    y: f32::from(y),
                                },
                                size: gpui::Size {
                                    width: f32::from(w),
                                    height: f32::from(h),
                                },
                            },
                        ));
                        cx.notify();
                    }
                } else if let Some((page_index, local_x, local_y)) =
                    self.content_to_page_coords(event.position.x, event.position.y, window)
                    && let Some(char_idx) =
                        self.find_char_at_position(page_index, local_x, local_y, window)
                {
                    self.update_selection_end(page_index, char_idx, cx);
                }
            }
        }
    }

    pub(crate) fn handle_root_mouse_up(&mut self, cx: &mut Context<Self>) {
        let was_dragging_resizer = self.dragging_left_resizer || self.dragging_right_resizer;

        self.is_dragging_scrollbar = false;
        self.is_dragging_thumbnail_scrollbar = false;
        self.dragging_left_resizer = false;
        self.dragging_right_resizer = false;
        self.is_panning = false;
        self.dragging_pin = None;
        self.resizing_pin = None;

        if let Some(drag) = self.annotation_drag.take() {
            if let Some(anns) = self.annotation_state.annotations.get(&drag.page)
                && let Some(ann) = anns.iter().find(|a| a.id == drag.annotation_id)
            {
                if let Some(ref delegate) = self.delegate {
                    delegate.save_annotation(ann);
                }
                self.annotation_version += 1;
            }
            cx.notify();
        }

        if was_dragging_resizer {
            self.save_current_state(Some(cx));
        }
    }

    pub(crate) fn handle_content_mouse_up(
        &mut self,
        mouse_pos: gpui::Point<gpui::Pixels>,
        window: &gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(drag) = self.annotation_drag.take() {
            if let Some(anns) = self.annotation_state.annotations.get(&drag.page)
                && let Some(ann) = anns.iter().find(|a| a.id == drag.annotation_id)
            {
                if let Some(ref delegate) = self.delegate {
                    delegate.save_annotation(ann);
                }
                self.annotation_version += 1;
            }
            self.is_mouse_down = false;
            cx.notify();
            return;
        }

        if self.is_mouse_down && !self.is_selecting {
            if self.overlay_button_clicked {
                self.overlay_button_clicked = false;
                self.is_mouse_down = false;
                cx.notify();
                return;
            }

            if let Some(pos) = self.mouse_down_pos {
                if let Some((page_index, px_x, px_y)) =
                    self.content_to_page_coords(pos.x, pos.y, window)
                {
                    if let Some(url) =
                        self.hit_test_link(page_index, f32::from(px_x), f32::from(px_y))
                    {
                        self.annotation_state.selected_id = None;
                        self.selection_start = None;
                        self.selection_end = None;
                        self.selected_text = None;
                        self.is_mouse_down = false;
                        self.mouse_down_pos = None;
                        if let Some(delegate) = &self.delegate {
                            delegate.on_link_click(url);
                        }
                        cx.notify();
                        return;
                    }

                    self.annotation_state.selected_id = self.hit_test_annotation(
                        page_index,
                        f32::from(px_x),
                        f32::from(px_y),
                        window,
                    );
                } else {
                    self.annotation_state.selected_id = None;
                }
            }

            self.selection_start = None;
            self.selection_end = None;
            self.selected_text = None;
            self.rect_in_progress = None;

            self.annotation_state.toolbar = None;
            self.annotation_state.context_menu = None;
            self.pin_context_menu = None;
            self.thumbnail_context_menu = None;
            self.annotation_state.note_editor = None;
            self.note_input_state = None;
            self.note_input_sub = None;
            cx.notify();
        }

        if self.is_selecting {
            // ─── PiP 图钉创建 ────────────────────────────────────
            if self.annotation_state.active_tool == crate::AnnotationTool::Pin {
                if let Some((page, bounds)) = self.rect_in_progress.take() {
                    let rem_size = f32::from(window.rem_size());
                    let display_w = PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size;
                    let (pdf_w, _pdf_h) = self
                        .page_sizes
                        .get(page as usize)
                        .copied()
                        .unwrap_or((612.0, 792.0));
                    let pdf_pw = pdf_w.max(1.0);

                    // 将显示坐标转换成 PDF 坐标（points）
                    let scale_to_pdf = pdf_pw / display_w;
                    let bx0 = bounds.origin.x * scale_to_pdf;
                    let by0 = bounds.origin.y * scale_to_pdf;
                    let bx1 = (bounds.origin.x + bounds.size.width) * scale_to_pdf;
                    let by1 = (bounds.origin.y + bounds.size.height) * scale_to_pdf;

                    // 从原始图像裁剪作为初始占位
                    let img_src =
                        if let Some(Some(raw_img)) = self.raw_page_images.get(page as usize) {
                            let scale_x = raw_img.width() as f32 / display_w;
                            let crop_x = (bounds.origin.x * scale_x).max(0.0) as u32;
                            let crop_y = (bounds.origin.y * scale_x).max(0.0) as u32;
                            let crop_w = (bounds.size.width * scale_x).max(1.0) as u32;
                            let crop_h = (bounds.size.height * scale_x).max(1.0) as u32;
                            let crop_w = crop_w.min(raw_img.width() - crop_x);
                            let crop_h = crop_h.min(raw_img.height() - crop_y);
                            if crop_w > 0 && crop_h > 0 {
                                let dynamic_img = image::DynamicImage::from((**raw_img).clone());
                                let cropped = dynamic_img
                                    .crop_imm(crop_x, crop_y, crop_w, crop_h)
                                    .to_rgba8();
                                let frame = image::Frame::new(cropped);
                                let render_img = gpui::RenderImage::new(smallvec::smallvec![frame]);
                                Some(gpui::ImageSource::Render(std::sync::Arc::new(render_img)))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                    // 将 window 坐标系转换为 main-view 容器坐标系
                    let toolbar_h =
                        f32::from(gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size()));
                    let tabbar_h = self.tab_bar_offset_rems * rem_size;
                    let sidebar_w = if self.is_left_sidebar_open {
                        f32::from(self.left_sidebar_width)
                    } else {
                        0.0
                    };

                    let (wx, wy) = match self.mouse_down_pos {
                        Some(down) => (
                            f32::from(down.x).min(f32::from(mouse_pos.x)),
                            f32::from(down.y).min(f32::from(mouse_pos.y)),
                        ),
                        None => (f32::from(mouse_pos.x), f32::from(mouse_pos.y)),
                    };

                    let pin_pos = gpui::Point {
                        x: gpui::px(wx - sidebar_w),
                        y: gpui::px(wy - tabbar_h - toolbar_h),
                    };

                    let pin_id = Uuid::new_v4().to_string();
                    let bbox = (bx0, by0, bx1, by1);

                    // 发送首次渲染请求（分辨率基于初始 CSS 尺寸，独立于 PDF zoom）
                    let init_w = bounds.size.width;
                    let bbox_w = (bx1 - bx0).max(1.0);
                    let render_scale = init_w * self.window_scale_factor * 1.2 / bbox_w;
                    self.pdf_service
                        .send_render_pin(page, pin_id.clone(), bbox, render_scale);

                    // 创建 PiP 图钉
                    let pin = crate::view::components::pip::PiPPin {
                        id: pin_id,
                        page,
                        bbox,
                        position: pin_pos,
                        size: gpui::Size {
                            width: gpui::px(bounds.size.width),
                            height: gpui::px(bounds.size.height),
                        },
                        image_source: img_src,
                        raw_image: None,
                    };
                    self.pins.push(pin);
                    self.annotation_state.active_tool = crate::AnnotationTool::Select;
                    cx.notify();
                }
            } else if let crate::AnnotationTool::Rectangle(color) =
                self.annotation_state.active_tool
            {
                if let Some((page, bounds)) = self.rect_in_progress.take() {
                    let rem_size = f32::from(window.rem_size());
                    let (display_w, display_h) = helpers::page_display_size(
                        &self.page_sizes,
                        page as usize,
                        self.zoom_level,
                        rem_size,
                    );

                    let id = Uuid::new_v4().to_string();
                    let now = Utc::now().timestamp();
                    let annotation = crate::Annotation {
                        id: id.clone(),
                        document_id: self.document_id.clone(),
                        page,
                        kind: crate::AnnotationKind::Rectangle {
                            x: bounds.origin.x / display_w,
                            y: bounds.origin.y / display_h,
                            w: bounds.size.width / display_w,
                            h: bounds.size.height / display_h,
                        },
                        color,
                        range: None,
                        note: None,
                        created_at: now,
                        updated_at: now,
                        version: 1,
                        is_deleted: false,
                        is_dirty: true,
                    };
                    self.annotation_state
                        .annotations
                        .entry(page)
                        .or_default()
                        .push(annotation.clone());
                    if let Some(delegate) = &self.delegate {
                        delegate.save_annotation(&annotation);
                    }
                    self.annotation_version += 1;
                    // 矩形注释自动退出
                    self.annotation_state.active_tool = crate::AnnotationTool::Select;
                    cx.notify();
                }
            } else if let (Some((sp, si)), Some((ep, ei))) =
                (self.selection_start, self.selection_end)
            {
                self.annotation_state.context_menu = None;
                self.pin_context_menu = None;
                self.thumbnail_context_menu = None;
                self.annotation_state.note_editor = None;
                self.note_input_state = None;
                self.note_input_sub = None;
                if (sp, si) == (ep, ei) {
                    self.annotation_state.toolbar = None;
                    self.selection_start = None;
                    self.selection_end = None;
                    self.selected_text = None;
                } else {
                    let (start_page, start_char, end_page, end_char) =
                        if sp < ep || (sp == ep && si <= ei) {
                            (sp, si, ep, ei)
                        } else {
                            (ep, ei, sp, si)
                        };
                    self.annotation_state.toolbar = Some(crate::AnnotationToolbarState {
                        start_page,
                        start_char,
                        end_page,
                        end_char,
                    });
                }
            }
            self.end_selection(cx);
        }
        self.is_mouse_down = false;
        self.mouse_down_pos = None;
        self.rect_in_progress = None;
    }

    pub(crate) fn find_char_at_position(
        &mut self,
        page_index: u16,
        x: Pixels,
        y: Pixels,
        window: &Window,
    ) -> Option<usize> {
        let text_data = self
            .page_text_data
            .get(page_index as usize)
            .and_then(|d| d.as_ref())?;
        let screen_x = f32::from(x);
        let screen_y = f32::from(y);

        // 若 text_data 与当前 zoom 不匹配（异步文本拉取尚未完成），
        // 将点击坐标反向缩放到 text_data.display_w 坐标空间再查找
        let rem_size = window.rem_size();
        let rem_size_px = f32::from(rem_size);
        let current_display_w = PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size_px;
        let (search_x, search_y) = if (text_data.display_w - current_display_w).abs() > 0.001 {
            let scale = text_data.display_w / current_display_w;
            (screen_x * scale, screen_y * scale)
        } else {
            (screen_x, screen_y)
        };

        // 缓存不存在时构建 Y 分桶索引，避免每次全量 O(n) 扫描
        let bucket_height = 20.0;
        let buckets = self.find_char_cache.entry(page_index).or_insert_with(|| {
            let max_y = text_data
                .chars
                .iter()
                .map(|c| c.y + c.height)
                .fold(0.0f32, f32::max);
            let num_buckets = ((max_y / bucket_height).ceil() as usize).max(1);
            let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); num_buckets];
            for (idx, ch) in text_data.chars.iter().enumerate() {
                let bucket = ((ch.y + ch.height / 2.0) / bucket_height) as usize;
                if bucket < buckets.len() {
                    buckets[bucket].push(idx);
                }
            }
            buckets
        });

        let center_bucket = (search_y / bucket_height) as usize;
        let start_bucket = center_bucket.saturating_sub(1);
        let end_bucket = (center_bucket + 1).min(buckets.len().saturating_sub(1));

        let mut best_idx: Option<usize> = None;
        let mut best_dist = f32::MAX;

        for bucket in start_bucket..=end_bucket {
            for &idx in &buckets[bucket] {
                if let Some(ch) = text_data.chars.get(idx)
                    && !ch.char.is_whitespace()
                    && ch.x <= search_x
                    && search_x <= ch.x + ch.width
                    && ch.y <= search_y
                    && search_y <= ch.y + ch.height
                {
                    let center_x = ch.x + ch.width / 2.0;
                    let center_y = ch.y + ch.height / 2.0;
                    let dist =
                        ((center_x - search_x).powi(2) + (center_y - search_y).powi(2)).sqrt();
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = Some(idx);
                    }
                }
            }
        }

        best_idx
    }

    pub(crate) fn update_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(((sp, si), (ep, ei))) = self.selection_start.zip(self.selection_end) {
            if sp == ep && si == ei {
                self.selected_text = None;
                cx.notify();
                return;
            }
            let (start_page, start_char, end_page, end_char) = if sp < ep || (sp == ep && si <= ei)
            {
                (sp, si, ep, ei)
            } else {
                (ep, ei, sp, si)
            };

            let mut selected = String::new();
            let mut prev_y_and_height: Option<(f32, f32)> = None;

            for page in start_page..=end_page {
                let Some(data) = self
                    .page_text_data
                    .get(page as usize)
                    .and_then(|d| d.as_ref())
                else {
                    self.selected_text = None;
                    cx.notify();
                    return;
                };

                if page > start_page && !selected.is_empty() && !selected.ends_with('\n') {
                    selected.push('\n');
                    prev_y_and_height = None;
                }

                let range_start = if page == start_page { start_char } else { 0 };
                let range_end = if page == end_page {
                    end_char
                } else {
                    data.chars.len().saturating_sub(1)
                };

                if range_start <= range_end && range_end < data.chars.len() {
                    for ch in &data.chars[range_start..=range_end] {
                        if let Some((prev_y, prev_height)) = prev_y_and_height {
                            let threshold = prev_height.max(ch.height).max(1.0) * 0.5;
                            if (ch.y - prev_y).abs() > threshold {
                                selected.push('\n');
                            }
                        }
                        selected.push(ch.char);
                        prev_y_and_height = Some((ch.y, ch.height));
                    }
                }
            }

            self.selected_text = Some(selected);
        }
        cx.notify();
    }

    pub(crate) fn start_selection(
        &mut self,
        page_index: u16,
        char_index: usize,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        self.selection_start = Some((page_index, char_index));
        self.selection_end = Some((page_index, char_index));
        self.update_selection(cx);
    }

    pub(crate) fn update_selection_end(
        &mut self,
        page_index: u16,
        char_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.is_selecting {
            self.selection_end = Some((page_index, char_index));
            self.update_selection(cx);
        }
    }

    pub(crate) fn end_selection(&mut self, cx: &mut Context<Self>) {
        self.is_selecting = false;

        match self.annotation_state.active_tool {
            crate::AnnotationTool::Select => {
                if self.is_right_sidebar_open
                    && let Some(ref text) = self.selected_text
                    && !text.is_empty()
                {
                    if self.auto_translate {
                        self.translate_text(text.clone(), false, cx);
                    } else {
                        let prev_translated = self
                            .translation_result
                            .as_ref()
                            .and_then(|r| r.translated.clone());
                        self.translation_result = Some(TranslationResult {
                            original: text.clone(),
                            translated: prev_translated,
                            is_loading: false,
                            error: None,
                        });
                        cx.notify();
                    }
                }
            }
            crate::AnnotationTool::Highlight(_) | crate::AnnotationTool::Underline(_) => {
                // 延迟到浮动工具栏处理
            }
            crate::AnnotationTool::Rectangle(_) | crate::AnnotationTool::Pin => {
                // Creation is already handled in handle_mouse_up before end_selection
            }
        }
    }

    pub(crate) fn content_to_page_coords(
        &self,
        content_x: Pixels,
        content_y: Pixels,
        window: &Window,
    ) -> Option<(u16, Pixels, Pixels)> {
        let rem_size = window.rem_size();
        let rem_size_px = f32::from(rem_size);

        let toolbar_height = gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(rem_size);
        let tab_bar_h_px = self.tab_bar_offset_rems * rem_size_px;
        let content_y_px = f32::from(content_y) - tab_bar_h_px - f32::from(toolbar_height);

        let scroll_top = self.list_state.logical_scroll_top();
        let display_width_px = PAGE_BASE_WIDTH_REMS * self.zoom_level * rem_size_px;

        let mut available_width = f32::from(window.viewport_size().width);
        let mut offset_x = 0.0;
        if self.is_left_sidebar_open {
            let w = f32::from(self.left_sidebar_width);
            available_width -= w;
            offset_x = w;
        }
        if self.is_right_sidebar_open {
            available_width -= f32::from(self.right_sidebar_width);
        }
        let center_offset_x = offset_x + (available_width - display_width_px) / 2.0 + self.offset_x;

        let adjusted_y = content_y_px + f32::from(scroll_top.offset_in_item);

        let mut accumulated_height = 0.0;
        for ix in scroll_top.item_ix..self.total_pages {
            let page_height_px =
                helpers::page_height(&self.page_sizes, ix, self.zoom_level, rem_size_px);
            let item_height_px = page_height_px;

            if accumulated_height + item_height_px > adjusted_y {
                let local_y_px = adjusted_y - accumulated_height;
                let local_x_px = f32::from(content_x) - center_offset_x;

                if local_y_px >= 0.0
                    && local_y_px <= page_height_px
                    && local_x_px >= 0.0
                    && local_x_px <= display_width_px
                {
                    return Some((ix as u16, px(local_x_px), px(local_y_px)));
                } else {
                    return None;
                }
            }
            accumulated_height += item_height_px;
        }

        None
    }

    pub(crate) fn hit_test_link(&mut self, page_index: u16, x: f32, y: f32) -> Option<String> {
        let link_data = self
            .page_link_data
            .get(page_index as usize)
            .and_then(|d| d.as_ref())?;
        for link in &link_data.links {
            if x >= link.left && x <= link.right && y >= link.top && y <= link.bottom {
                return Some(link.url.clone());
            }
        }
        None
    }
    pub(crate) fn hit_test_annotation(
        &mut self,
        page_index: u16,
        x: f32,
        y: f32,
        window: &Window,
    ) -> Option<String> {
        let text_data = self
            .page_text_data
            .get(page_index as usize)
            .and_then(|d| d.as_ref())?;

        let rem_size = f32::from(window.rem_size());
        let (display_w, display_h) = helpers::page_display_size(
            &self.page_sizes,
            page_index as usize,
            self.zoom_level,
            rem_size,
        );

        let anns_on_page = self.annotation_state.annotations.get(&page_index);
        let anns_spanning = self
            .annotation_state
            .annotations
            .iter()
            .filter(|(p, _)| **p < page_index);

        let check_ann = |ann: &crate::Annotation| -> Option<String> {
            if ann.is_deleted {
                return None;
            }
            match &ann.kind {
                crate::AnnotationKind::Highlight | crate::AnnotationKind::Underline => {
                    if let Some(range) = &ann.range {
                        if page_index < range.start_page || page_index > range.end_page_or() {
                            return None;
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
                            return None;
                        }
                        for (bx, by, b_max_x, b_max_y) in text_data.merge_char_blocks(start, end) {
                            if x >= bx && x <= b_max_x && y >= by && y <= b_max_y {
                                return Some(ann.id.clone());
                            }
                        }
                    }
                }
                crate::AnnotationKind::Rectangle {
                    x: rx,
                    y: ry,
                    w: rw,
                    h: rh,
                } => {
                    if ann.page != page_index {
                        return None;
                    }
                    let ax = rx * display_w;
                    let ay = ry * display_h;
                    let aw = rw * display_w;
                    let ah = rh * display_h;
                    if x >= ax && x <= ax + aw && y >= ay && y <= ay + ah {
                        let dist_left = x - ax;
                        let dist_right = (ax + aw) - x;
                        let dist_top = y - ay;
                        let dist_bottom = (ay + ah) - y;
                        let edge_threshold = 8.0;
                        if dist_left <= edge_threshold
                            || dist_right <= edge_threshold
                            || dist_top <= edge_threshold
                            || dist_bottom <= edge_threshold
                        {
                            return Some(ann.id.clone());
                        }
                    }
                }
            }
            None
        };

        if let Some(anns) = anns_on_page {
            for ann in anns {
                if let Some(id) = check_ann(ann) {
                    return Some(id);
                }
            }
        }

        for (_, anns) in anns_spanning {
            for ann in anns {
                if !ann.is_deleted
                    && let Some(ref range) = ann.range
                    && range.end_page_or() >= page_index
                    && let Some(id) = check_ann(ann)
                {
                    return Some(id);
                }
            }
        }

        None
    }

    pub(crate) fn adjust_context_menu_position(
        &self,
        pos: Point<Pixels>,
        window: &Window,
    ) -> Point<Pixels> {
        let rem_size = window.rem_size();
        let toolbar_height = gpui::rems(TOOLBAR_HEIGHT_REMS).to_pixels(rem_size);
        let tab_bar_h = self.tab_bar_offset_rems * f32::from(rem_size);

        Point {
            x: pos.x,
            y: px(f32::from(pos.y) - tab_bar_h - f32::from(toolbar_height)),
        }
    }
}
