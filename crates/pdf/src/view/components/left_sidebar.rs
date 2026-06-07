use crate::TextPageData;
use crate::view::PdfReaderView;
use crate::view::types::{
    LeftSidebarTab, PageColorMode, PdfIconName, SearchMatch, SearchResultsDelegate,
    TOOLBAR_HEIGHT_REMS,
};
use gpui::prelude::*;
use gpui::{
    Context, Div, InteractiveElement, MouseButton, MouseDownEvent, Window, div, img, px, relative,
    rems,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::list::{List, ListEvent};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{ActiveTheme, Icon, Selectable, h_flex, label::Label, v_flex};
use i18n::I18nKey;
use std::sync::Arc;

struct FlattenedOutlineItem {
    id: String,
    title: String,
    page_index: u16,
    depth: usize,
    has_children: bool,
}

impl PdfReaderView {
    pub(crate) fn render_left_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();

        v_flex()
            .w(self.left_sidebar_width)
            .flex_shrink_0()
            .h_full()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.background)
            .child(
                // 侧边栏 Tab 切换
                h_flex()
                    .w_full()
                    .h_9()
                    .border_b_1()
                    .border_color(theme.border)
                    .justify_around()
                    .items_center()
                    .child(
                        Button::new("tab-thumbnails")
                            .ghost()
                            .icon(PdfIconName::Pages)
                            .when(
                                self.active_left_sidebar_tab == LeftSidebarTab::Thumbnails,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_left_sidebar_tab = LeftSidebarTab::Thumbnails;
                                this.search_text_storage = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tab-outline")
                            .ghost()
                            .icon(PdfIconName::Outline)
                            .when(
                                self.active_left_sidebar_tab == LeftSidebarTab::Outline,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_left_sidebar_tab = LeftSidebarTab::Outline;
                                this.search_text_storage = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tab-annotations")
                            .ghost()
                            .icon(PdfIconName::Annotations)
                            .when(
                                self.active_left_sidebar_tab == LeftSidebarTab::Annotations,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_left_sidebar_tab = LeftSidebarTab::Annotations;
                                this.search_text_storage = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tab-search")
                            .ghost()
                            .icon(PdfIconName::Search)
                            .when(
                                self.active_left_sidebar_tab == LeftSidebarTab::Search,
                                |b| b.selected(true),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.active_left_sidebar_tab = LeftSidebarTab::Search;
                                if this.search_state.is_some() {
                                    this.ensure_search_text_storage();
                                    if let Some(ref state) = this.search_state
                                        && !state.query.is_empty()
                                    {
                                        this.re_run_search_from_storage(cx);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(
                // 侧边栏内容
                div()
                    .size_full()
                    .overflow_hidden()
                    .child(match self.active_left_sidebar_tab {
                        LeftSidebarTab::Thumbnails => {
                            self.render_thumbnail_list(window, cx).into_any_element()
                        }
                        LeftSidebarTab::Outline => self.render_outline_list(cx).into_any_element(),
                        LeftSidebarTab::Annotations => {
                            self.render_annotation_list(cx).into_any_element()
                        }
                        LeftSidebarTab::Search => {
                            self.render_search_content(window, cx).into_any_element()
                        }
                    }),
            )
    }

    pub(crate) fn get_thumbnail_item_height(&self) -> f32 {
        let current_w = f32::from(self.left_sidebar_width);
        let padding = 40.0;
        let max_container_h = 240.0;
        let container_ratio = 1.35; // 接近 A4 比例

        // 计算当前侧边栏宽度对应的理想高度
        let ideal_h = (current_w - padding) * container_ratio;

        // 最终高度受限于最大高度
        let final_h = ideal_h.min(max_container_h);

        final_h + 20.0 // 加上页码间距 (减小至 20px)
    }

    pub(crate) fn render_thumbnail_list(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view_weak = cx.entity().downgrade();
        div()
            .size_full()
            .relative()
            .child(
                gpui::list(self.thumbnail_list_state.clone(), move |index, _, cx| {
                    view_weak
                        .update(cx, |this, cx| {
                            this.render_thumbnail_item(index as u16, cx)
                                .into_any_element()
                        })
                        .unwrap_or_else(|_| div().into_any_element())
                })
                .size_full(),
            )
            .child(self.render_thumbnail_scrollbar(window, cx))
    }

    pub(crate) fn render_thumbnail_scrollbar(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        if self.total_pages > 0 {
            let scroll_top = self.thumbnail_list_state.logical_scroll_top();
            let current_ix = scroll_top.item_ix;

            let item_height_px = self.get_thumbnail_item_height();
            let total_height_px = self.total_pages as f32 * item_height_px;

            let view_height_px = f32::from(window.viewport_size().height);
            // 侧边栏高度需要减去 Tab 栏高度 (h-9 = 2.25rem = 36px)
            let sidebar_content_height_px = view_height_px - 36.0;

            let scrollable_height_px = (total_height_px - sidebar_content_height_px).max(0.0);
            let current_scroll_px =
                (current_ix as f32 * item_height_px) + f32::from(scroll_top.offset_in_item.abs());

            let scroll_ratio = if scrollable_height_px > 0.0 {
                (current_scroll_px / scrollable_height_px).clamp(0.0, 1.0)
            } else {
                0.0
            };

            let thumb_height_pct = (sidebar_content_height_px
                / total_height_px.max(sidebar_content_height_px))
            .clamp(0.05, 1.0);
            let track_avail_pct = 1.0 - thumb_height_pct;
            let thumb_top_pct = scroll_ratio * track_avail_pct;

            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(px(12.0))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(
                        move |this: &mut Self,
                              event: &MouseDownEvent,
                              _window: &mut Window,
                              cx: &mut Context<Self>| {
                            let thumb_height_px = sidebar_content_height_px * thumb_height_pct;
                            this.thumbnail_drag_offset = thumb_height_px / 2.0;
                            this.is_dragging_thumbnail_scrollbar = true;
                            this.scroll_thumbnails_to_position(
                                event.position.y,
                                sidebar_content_height_px,
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
                        .w(px(4.0))
                        .h(relative(thumb_height_pct))
                        .bg(theme.scrollbar_thumb.opacity(0.6))
                        .rounded_full()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(
                                move |this: &mut Self,
                                      event: &MouseDownEvent,
                                      _window: &mut Window,
                                      cx: &mut Context<Self>| {
                                    cx.stop_propagation();
                                    this.is_dragging_thumbnail_scrollbar = true;
                                    let mouse_y_rel = f32::from(event.position.y) - 36.0; // 减去 Tab 栏高度
                                    let thumb_top_px = sidebar_content_height_px * thumb_top_pct;
                                    this.thumbnail_drag_offset = mouse_y_rel - thumb_top_px;
                                },
                            ),
                        ),
                )
        } else {
            div()
        }
    }

    pub(crate) fn render_thumbnail_item(
        &mut self,
        page_index: u16,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let is_current = self.current_page == page_index;

        // 统一尺寸逻辑
        let current_sidebar_w = f32::from(self.left_sidebar_width);
        let max_container_h = 240.0;
        let container_ratio = 1.35;
        let padding = 40.0;

        // 计算容器尺寸
        let container_h = ((current_sidebar_w - padding) * container_ratio).min(max_container_h);
        let container_w = container_h / container_ratio;

        let item_height = self.get_thumbnail_item_height();

        // 获取页面真实比例
        let (pdf_w, pdf_h) = self
            .page_sizes
            .get(page_index as usize)
            .copied()
            .unwrap_or((612.0, 792.0));
        let aspect_ratio = pdf_h / pdf_w;

        // 在固定比例容器内缩放图片 (Fit-in)
        let thumb_w;
        let thumb_h;
        if aspect_ratio > container_ratio {
            thumb_h = container_h;
            thumb_w = thumb_h / aspect_ratio;
        } else {
            thumb_w = container_w;
            thumb_h = thumb_w * aspect_ratio;
        }

        v_flex()
            .id(("thumbnail-item", page_index as usize))
            .w_full()
            .h(px(item_height))
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.scroll_to_page(page_index, px(0.0), cx);
            }))
            .child(
                // 固定比例的卡片容器
                div()
                    .relative()
                    .w(px(container_w))
                    .h(px(container_h))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        // 实际的缩略图内容
                        div()
                            .relative()
                            .w(px(thumb_w))
                            .h(px(thumb_h))
                            .bg(match self.page_color_mode {
                                PageColorMode::White => gpui::white(),
                                PageColorMode::Sepia => gpui::rgb(0xF4ECD8).into(),
                                PageColorMode::EyeProtect => gpui::rgb(0xCCE8CF).into(),
                            })
                            .shadow_sm()
                            .child(div().size_full().overflow_hidden().child(
                                match self.thumbnail_cache.get(&page_index) {
                                    Some(img_src) => {
                                        img(img_src.clone()).size_full().into_any_element()
                                    }
                                    None => {
                                        if !self.pending_thumbnails.contains(&page_index) {
                                            self.pending_thumbnails.insert(page_index);
                                            self.pdf_service.send_thumbnail_render(
                                                page_index,
                                                250.0,
                                                self.render_generation,
                                            );
                                        }
                                        div().size_full().into_any_element()
                                    }
                                },
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .when(is_current, |s: Div| {
                                        s.border_3().border_color(theme.primary)
                                    })
                                    .when(!is_current, |s: Div| {
                                        s.border_1().border_color(theme.border.opacity(0.3))
                                    })
                                    .rounded(px(2.0)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom_1()
                                    .right_1()
                                    .px_1()
                                    .rounded(px(2.0))
                                    .bg(if is_current {
                                        theme.primary
                                    } else {
                                        theme.background.opacity(0.8)
                                    })
                                    .child(
                                        Label::new(format!("{}", page_index + 1))
                                            .text_xs()
                                            .when(is_current, |l: Label| {
                                                l.text_color(theme.primary_foreground)
                                                    .font_weight(gpui::FontWeight::BOLD)
                                            })
                                            .when(!is_current, |l: Label| {
                                                l.text_color(theme.foreground)
                                            }),
                                    ),
                            ),
                    ),
            )
    }

    pub(crate) fn render_outline_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        if self.outlines.is_none() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Label::new(i18n::t(I18nKey::LoadingOutline, self.language))
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }

        let mut flattened = Vec::new();
        if let Some(outlines) = &self.outlines {
            self.flatten_outlines(outlines, 0, "", &mut flattened);
        }

        if flattened.is_empty() {
            return v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Label::new(i18n::t(I18nKey::NoOutline, self.language))
                        .text_color(theme.muted_foreground),
                )
                .into_any_element();
        }

        let list_items = flattened
            .into_iter()
            .map(|item| {
                let is_expanded = self.expanded_outlines.contains(&item.id);
                let page = item.page_index;
                let id = item.id.clone();

                let arrow_id = id.clone();
                let text_id = id.clone();
                let container_id = id.clone();

                v_flex()
                    .w_full()
                    .id(gpui::SharedString::from(format!(
                        "outline-{}",
                        container_id
                    )))
                    .child(
                        h_flex()
                            .w_full()
                            .p_1()
                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                            .cursor_pointer()
                            .child(
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "outline-arrow-{}",
                                        arrow_id
                                    )))
                                    .w(px(16.0))
                                    .h(px(16.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(item.has_children, |this| {
                                        let arrow_id = arrow_id.clone();
                                        this.on_click(cx.listener(move |this, _, _, cx| {
                                            if this.expanded_outlines.contains(&arrow_id) {
                                                this.expanded_outlines.remove(&arrow_id);
                                            } else {
                                                this.expanded_outlines.insert(arrow_id.clone());
                                            }
                                            cx.notify();
                                        }))
                                        .child(
                                            Icon::new(if is_expanded {
                                                PdfIconName::ChevronDown
                                            } else {
                                                PdfIconName::ChevronRight
                                            })
                                            .size(px(10.0)),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "outline-text-{}",
                                        text_id
                                    )))
                                    .flex_grow()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.scroll_to_page(page, px(0.0), cx);
                                    }))
                                    .child(
                                        Label::new(item.title)
                                            .text_sm()
                                            .text_color(theme.foreground),
                                    ),
                            )
                            .pl(px(item.depth as f32 * 12.0)),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        div()
            .size_full()
            .overflow_y_scrollbar()
            .child(v_flex().w_full().p_2().children(list_items))
            .into_any_element()
    }

    fn flatten_outlines(
        &self,
        items: &[crate::OutlineItem],
        depth: usize,
        parent_id: &str,
        result: &mut Vec<FlattenedOutlineItem>,
    ) {
        for (i, item) in items.iter().enumerate() {
            let id = if parent_id.is_empty() {
                format!("{}-{}", i, item.title)
            } else {
                format!("{}/{}-{}", parent_id, i, item.title)
            };
            let has_children = !item.children.is_empty();
            let is_expanded = self.expanded_outlines.contains(&id);

            result.push(FlattenedOutlineItem {
                id: id.clone(),
                title: item.title.clone(),
                page_index: item.page_index,
                depth,
                has_children,
            });

            if has_children && is_expanded {
                self.flatten_outlines(&item.children, depth + 1, &id, result);
            }
        }
    }

    pub(crate) fn render_annotation_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let mut annotations: Vec<_> = self
            .annotation_state
            .annotations
            .values()
            .flatten()
            .filter(|a| !a.is_deleted)
            .cloned()
            .collect();

        // 按页码和时间排序
        annotations.sort_by(|a, b| a.page.cmp(&b.page).then(a.created_at.cmp(&b.created_at)));

        let editing_id = self.editing_note_sidebar_id.clone();
        let editing_input = self.editing_note_sidebar_input.clone();

        let list_items = annotations
            .into_iter()
            .map(|ann| {
                let kind_text = match ann.kind {
                    crate::AnnotationKind::Highlight => i18n::t(I18nKey::Highlight, self.language),
                    crate::AnnotationKind::Underline => i18n::t(I18nKey::Underline, self.language),
                    crate::AnnotationKind::Rectangle { .. } => i18n::t(I18nKey::RectangleAnnotation, self.language),
                };

                let color_rgba = match ann.color {
                    crate::AnnotationColor::Yellow => gpui::rgba(0xFFD400FF),
                    crate::AnnotationColor::Red => gpui::rgba(0xFF6666FF),
                    crate::AnnotationColor::Green => gpui::rgba(0x5FB236FF),
                    crate::AnnotationColor::Blue => gpui::rgba(0x2EA8E5FF),
                    crate::AnnotationColor::Purple => gpui::rgba(0xA28AE5FF),
                    crate::AnnotationColor::Magenta => gpui::rgba(0xE56EEEFF),
                    crate::AnnotationColor::Orange => gpui::rgba(0xF19837FF),
                    crate::AnnotationColor::Gray => gpui::rgba(0xAAAAAAFF),
                };

                let page = ann.page;
                let ann_page_range = ann.range.as_ref().map(|r| r.end_page_or());
                let ann_id_left = ann.id.clone();
                let ann_id = ann.id.clone();
                let start_char = ann.range.as_ref().map(|r| r.start_char);
                let end_char = ann.range.as_ref().map(|r| r.end_char);
                let range_start_page = ann.range.as_ref().map(|r| r.start_page);
                let range_end_page = ann.range.as_ref().map(|r| r.end_page_or());
                let is_editing = editing_id.as_ref() == Some(&ann.id);
                let note_text = ann.note.clone().unwrap_or_default();

                v_flex()
                    .id(gpui::SharedString::from(ann.id.clone()))
                    .w_full()
                    .p_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let toolbar_height = f32::from(rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size()));
                        let content_height = f32::from(window.viewport_size().height) - toolbar_height;
                        let ann_id = ann_id_left.clone();
                        match (start_char, end_char, range_start_page, range_end_page) {
                            (Some(s), Some(e), Some(sp), Some(ep)) => {
                                // 文本已就绪 → 立即计算并跳转
                                if this.text_cache.get(&sp).is_some() && this.text_cache.get(&ep).is_some() {
                                    let (target_page, offset) = this.annotation_scroll_offset(
                                        sp, s, ep, e, content_height,
                                    );
                                    this.annotation_state.selected_id = Some(ann_id);
                                    this.scroll_to_page(target_page, offset, cx);
                                } else {
                                    // 缓存未命中 → 异步等文本到齐再跳
                                    this.annotation_state.selected_id = Some(ann_id.clone());
                                    cx.spawn(move |view: gpui::WeakEntity<PdfReaderView>, cx: &mut gpui::AsyncApp| {
                                        let mut cx = cx.clone();
                                        async move {
                                        let _ = view.update(&mut cx, |this, _| {
                                            if this.text_cache.get(&sp).is_none() {
                                                let display_w = crate::view::PAGE_BASE_WIDTH_REMS
                                                    * this.zoom_level * this.last_rem_size;
                                                let (pdf_w, pdf_h) = this
                                                    .page_sizes
                                                    .get(sp as usize)
                                                    .copied()
                                                    .unwrap_or((612.0, 792.0));
                                                this.pdf_service.send_text(
                                                    sp,
                                                    display_w,
                                                    display_w * (pdf_h / pdf_w),
                                                    this.render_generation,
                                                );
                                            }
                                            if this.text_cache.get(&ep).is_none() {
                                                let display_w = crate::view::PAGE_BASE_WIDTH_REMS
                                                    * this.zoom_level * this.last_rem_size;
                                                let (pdf_w, pdf_h) = this
                                                    .page_sizes
                                                    .get(ep as usize)
                                                    .copied()
                                                    .unwrap_or((612.0, 792.0));
                                                this.pdf_service.send_text(
                                                    ep,
                                                    display_w,
                                                    display_w * (pdf_h / pdf_w),
                                                    this.render_generation,
                                                );
                                            }
                                        });
                                        let start = std::time::Instant::now();
                                        loop {
                                            if start.elapsed() > std::time::Duration::from_secs(10) {
                                                break;
                                            }
                                            let ready = view
                                                .update(&mut cx, |this, _| {
                                                    this.text_cache.get(&sp).is_some()
                                                        && this.text_cache.get(&ep).is_some()
                                                })
                                                .unwrap_or(false);
                                            if ready {
                                                break;
                                            }
                                            cx.background_executor()
                                                .timer(std::time::Duration::from_millis(50))
                                                .await;
                                        }
                                        let _ = view.update(&mut cx, |this, cx| {
                                            let both_ready = this.text_cache.get(&sp).is_some()
                                                && this.text_cache.get(&ep).is_some();
                                            if both_ready {
                                                let (target_page, offset) = this.annotation_scroll_offset(
                                                    sp, s, ep, e, content_height,
                                                );
                                                this.scroll_to_page(target_page, offset, cx);
                                            } else {
                                                // 超时退化：跳到 start_page 顶部
                                                this.scroll_to_page(sp, px(0.0), cx);
                                            }
                                        });
                                        }
                                    })
                                    .detach();
                                }
                            }
                            _ => {
                                this.annotation_state.selected_id = Some(ann_id);
                                this.scroll_to_page(page, px(0.0), cx);
                            }
                        }
                    }))
                    .on_mouse_down(
                        gpui::MouseButton::Right,
                        cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                            let id = ann_id.clone();
                            this.annotation_state.selected_id = Some(id.clone());
                            this.annotation_state.context_menu = Some(crate::ContextMenuState {
                                annotation_id: id,
                                position: event.position,
                                from_sidebar: true,
                            });
                            cx.notify();
                        }),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div().w(px(12.0)).h(px(12.0)).rounded_full().bg(color_rgba),
                                    )
                                    .child(
                                        Label::new(kind_text)
                                            .text_sm()
                                            .text_color(theme.foreground),
                                    ),
                            )
                            .child(
                                Label::new(
                                    if ann_page_range.is_some_and(|ep| ep != page) {
                                        i18n::tf(I18nKey::PageRange, self.language, &[&(page + 1).to_string(), &(ann_page_range.unwrap() + 1).to_string()])
                                    } else {
                                        i18n::tf(I18nKey::SinglePage, self.language, &[&(page + 1).to_string()])
                                    }
                                )
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            ),
                    )
                    .when(is_editing, |this| {
                        if let Some(input) = &editing_input {
                            this.child(
                                v_flex()
                                    .w_full()
                                    .mt_1()
                                    .child(Input::new(input).w_full())
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .justify_end()
                                            .gap_2()
                                            .mt_1()
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .rounded_sm()
                                                    .hover(|s| s.bg(gpui::transparent_black().opacity(0.1)))
                                                    .child(Icon::new(PdfIconName::Check).size(px(16.0)))
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            this.save_sidebar_note(cx);
                                                        }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .cursor_pointer()
                                                    .rounded_sm()
                                                    .hover(|s| s.bg(gpui::transparent_black().opacity(0.1)))
                                                    .child(Icon::new(PdfIconName::Close).size(px(16.0)))
                                                    .on_mouse_down(
                                                        gpui::MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            this.cancel_sidebar_note(cx);
                                                        }),
                                                    ),
                                            ),
                                    ),
                            )
                        } else {
                            this
                        }
                    })
                    .when(!is_editing && !note_text.is_empty(), |this| {
                        this.child(
                            div()
                                .w_full()
                                .mt_1()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(note_text),
                        )
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        v_flex().size_full().child(
            div()
                .size_full()
                .overflow_y_scrollbar()
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.annotation_state.toolbar = None;
                        this.annotation_state.context_menu = None;
                        this.annotation_state.note_editor = None;
                        this.note_input_state = None;
                        this.note_input_sub = None;
                        cx.notify();
                    }),
                )
                .children(list_items),
        )
    }

    // ── 搜索方法 ──────────────────────────────────────────

    pub(crate) fn search_context_text(&mut self, m: &SearchMatch) -> String {
        let data = self
            .text_cache
            .get(&m.page_index)
            .map(Arc::as_ref)
            .or_else(|| {
                self.search_text_storage
                    .as_ref()
                    .and_then(|s| s.get(m.page_index as usize))
                    .and_then(|opt| opt.as_ref())
                    .map(Arc::as_ref)
            });

        Self::format_context_text(data, m)
    }

    fn format_context_text(data: Option<&TextPageData>, m: &SearchMatch) -> String {
        if let Some(data) = data {
            let ctx_start = m.start_char.saturating_sub(20);
            let ctx_end = (m.end_char + 20).min(data.chars.len());
            let mut snippet: String = data
                .chars
                .get(ctx_start..ctx_end)
                .map(|slice| slice.iter().map(|c| c.char).collect())
                .unwrap_or_default();
            snippet = snippet.replace('\r', " ");
            snippet = snippet.replace('\n', " ");
            if snippet.chars().count() > 50 {
                format!("{}...", snippet.chars().take(50).collect::<String>())
            } else {
                snippet
            }
        } else {
            String::new()
        }
    }

    pub(crate) fn render_search_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if self.search_input_state.is_none() {
            let entity = cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(i18n::t(I18nKey::SearchInputPlaceholder, self.language))
            });
            if let Some(ref state) = self.search_state {
                entity.update(cx, |s, cx| {
                    s.set_value(&state.query, window, cx);
                });
            }

            let sub = cx.subscribe(
                &entity,
                |this: &mut PdfReaderView,
                 _emitter: gpui::Entity<InputState>,
                 event: &InputEvent,
                 cx: &mut Context<PdfReaderView>| {
                    if let InputEvent::Change = event
                        && let Some(input) = &this.search_input_state
                    {
                        let text = input.read(cx).text().to_string();
                        this.perform_search(&text, cx);
                    }
                },
            );
            entity.update(cx, |state, inner_cx| {
                state.focus(window, inner_cx);
            });
            self.search_input_sub = Some(sub);
            self.search_input_state = Some(entity);
        }

        if self.search_list_state.is_none() {
            let delegate = SearchResultsDelegate {
                items: Vec::new(),
                active_match_idx: None,
                selected_idx: None,
            };
            let list_state =
                cx.new(|cx| gpui_component::list::ListState::new(delegate, window, cx));
            let sub = cx.subscribe(
                &list_state,
                |this: &mut PdfReaderView,
                 _: gpui::Entity<gpui_component::list::ListState<SearchResultsDelegate>>,
                 event: &ListEvent,
                 cx: &mut Context<PdfReaderView>| {
                    match event {
                        ListEvent::Confirm(ix) | ListEvent::Select(ix) => {
                            if let Some(ref mut state) = this.search_state {
                                state.active_match_idx = Some(ix.row);
                                if let Some(m) = state.active_match().cloned() {
                                    let (target_page, offset) = this.annotation_scroll_offset(
                                        m.page_index,
                                        m.start_char,
                                        m.page_index,
                                        m.end_char,
                                        this.search_content_height,
                                    );
                                    this.scroll_to_page(target_page, offset, cx);
                                }
                            }
                        }
                        _ => {}
                    }
                },
            );
            self.search_list_sub = Some(sub);
            self.search_list_state = Some(list_state);
        }

        let theme = cx.theme();
        let muted = theme.muted_foreground;

        let results_info = match &self.search_state {
            Some(state) if !state.query.is_empty() => {
                let total = state.total_matches();
                let current = state.active_match_idx.map(|i| i + 1).unwrap_or(0);
                Some(format!("{}/{}", current, total))
            }
            _ => None,
        };

        v_flex()
            .size_full()
            .child(
                v_flex()
                    .w_full()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_1()
                            .child(if let Some(input) = &self.search_input_state {
                                Input::new(input).w_full().into_any_element()
                            } else {
                                div().into_any_element()
                            })
                            .when_some(self.search_state.as_ref(), |this, state| {
                                if !state.query.is_empty() {
                                    this.child(
                                        Button::new("search-clear")
                                            .ghost()
                                            .icon(PdfIconName::Close)
                                            .compact()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                if let Some(input) = &this.search_input_state {
                                                    input.update(cx, |s, icx| {
                                                        s.set_value("", window, icx);
                                                    });
                                                }
                                                this.search_state = None;
                                                this.search_text_storage = None;
                                                if let Some(ls) = &this.search_list_state {
                                                    ls.update(cx, |s, _| {
                                                        s.delegate_mut().items.clear();
                                                    });
                                                }
                                                cx.notify();
                                            })),
                                    )
                                } else {
                                    this
                                }
                            }),
                    )
                    .when_some(results_info, |this, info| {
                        this.child(
                            h_flex()
                                .w_full()
                                .justify_center()
                                .child(Label::new(info).text_sm().text_color(muted)),
                        )
                    }),
            )
            .child(
                v_flex()
                    .flex_grow()
                    .h_0()
                    .w_full()
                    .px_3()
                    .pb_3()
                    .overflow_y_scrollbar()
                    .child({
                        if let Some(ref state) = self.search_state {
                            if !state.results.is_empty() {
                                if let Some(ref list_state) = self.search_list_state {
                                    List::new(list_state).flex_1().w_full().into_any_element()
                                } else {
                                    div().into_any_element()
                                }
                            } else if !state.query.is_empty() {
                                div().into_any_element()
                            } else {
                                v_flex()
                                    .size_full()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Label::new(i18n::t(I18nKey::SearchInPdf, self.language))
                                            .text_color(muted),
                                    )
                                    .into_any_element()
                            }
                        } else {
                            v_flex()
                                .size_full()
                                .items_center()
                                .justify_center()
                                .child(
                                    Label::new(i18n::t(I18nKey::SearchInPdf, self.language))
                                        .text_color(muted),
                                )
                                .into_any_element()
                        }
                    }),
            )
            .into_any_element()
    }
}
