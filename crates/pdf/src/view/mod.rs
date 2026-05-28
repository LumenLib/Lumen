use self::text_format::clean_translation_text;
use crate::{
    Annotation, AnnotationState, PdfInitialState, PdfReaderDelegate, PdfResponse, PdfService,
    TextPageData,
};
use gpui::prelude::*;
use gpui::{
    App, AsyncApp, ClipboardItem, Context, FocusHandle, Focusable, KeyDownEvent, ListAlignment,
    ListOffset, ListState, MouseButton, Render, WeakEntity, Window, div, px, rems,
};
use gpui_component::{ActiveTheme, Icon, button::Button, h_flex, label::Label, v_flex};
use i18n::{I18nKey, Language};
use log::{error, info};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

pub mod actions;
pub mod components;
pub mod selection;
pub mod text_format;
pub mod types;

pub use types::*;

pub struct PdfReaderView {
    pub(crate) pdf_service: Arc<PdfService>,
    pub(crate) delegate: Option<Arc<dyn PdfReaderDelegate>>,
    pub(crate) current_page: u16,
    pub(crate) current_offset_y: f32,
    pub(crate) total_pages: usize,
    pub(crate) page_sizes: Vec<(f32, f32)>,

    // 状态
    pub(crate) zoom_level: f32,
    pub(crate) render_zoom: f32,
    pub(crate) list_state: ListState,
    pub(crate) render_generation: u64,
    pub(crate) worker_state: WorkerState,
    pub(crate) initial_state: PdfInitialState,
    pub(crate) is_restoring: bool,
    pub(crate) last_rem_size: f32,
    pub(crate) last_zoom_level: f32,
    pub(crate) fit_to_width_mode: bool,

    pub(crate) page_cache: LruCache<u16, gpui::ImageSource>,
    pub(crate) stale_cache: LruCache<u16, gpui::ImageSource>,
    pub(crate) raw_page_cache: LruCache<u16, Arc<image::RgbaImage>>,
    pub(crate) text_cache: LruCache<u16, crate::TextPageData>,
    pub(crate) link_cache: LruCache<u16, crate::LinkPageData>,
    // 字符位置查找缓存（Y 分桶索引，避免全量 O(n) 扫描）
    pub(crate) find_char_cache: HashMap<u16, Vec<Vec<usize>>>,

    // 程序化滚动（缩放/恢复等），跳过当前帧的页面跟踪覆写
    pub(crate) programmatic_scroll: bool,

    // 选择状态
    pub(crate) is_mouse_down: bool,
    pub(crate) mouse_down_pos: Option<gpui::Point<gpui::Pixels>>,
    pub(crate) is_selecting: bool,
    pub(crate) is_panning: bool,
    pub(crate) offset_x: f32,
    pub(crate) selection_start: Option<(u16, usize)>, // (page_index, char_index)
    pub(crate) selection_end: Option<(u16, usize)>,   // (page_index, char_index)
    pub(crate) selected_text: Option<String>,
    pub(crate) rect_in_progress: Option<(u16, gpui::Bounds<f32>)>, // (page_index, bounds_in_pdf_coords)

    // 交互
    pub(crate) is_dragging_scrollbar: bool,
    pub(crate) drag_offset: f32,
    pub(crate) is_dragging_thumbnail_scrollbar: bool,
    pub(crate) thumbnail_drag_offset: f32,

    // 左侧边栏状态
    pub(crate) is_left_sidebar_open: bool,
    pub(crate) active_left_sidebar_tab: LeftSidebarTab,
    pub(crate) thumbnail_cache: LruCache<u16, gpui::ImageSource>,
    pub(crate) thumbnail_list_state: ListState,
    // 右侧边栏状态
    pub(crate) is_right_sidebar_open: bool,
    pub(crate) active_right_sidebar_tab: RightSidebarTab,
    pub(crate) translation_result: Option<TranslationResult>,
    pub(crate) is_engine_menu_open: bool,
    pub(crate) translation_original_expanded: bool,
    pub(crate) translation_font_size: f32,
    pub(crate) auto_translate: bool,

    pub(crate) document_id: String,
    // 侧边栏宽度与拖拽状态
    pub(crate) left_sidebar_width: gpui::Pixels,
    pub(crate) right_sidebar_width: gpui::Pixels,
    pub(crate) dragging_left_resizer: bool,
    pub(crate) dragging_right_resizer: bool,
    pub(crate) last_content_width: f32,

    pub(crate) annotation_state: AnnotationState,
    pub(crate) annotation_version: u64,
    pub(crate) last_composited_version: u64,

    pub(crate) focus_handle: FocusHandle,
    pub(crate) has_focused: bool,

    // 大纲状态
    pub(crate) outlines: Option<Vec<crate::OutlineItem>>,
    pub(crate) expanded_outlines: std::collections::HashSet<String>,

    // 笔记编辑器
    pub(crate) note_input_state: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) note_input_sub: Option<gpui::Subscription>,
    /// 防止 overlay 按钮点击后 main_content 的 mousedown+mouseup 连锁清除 overlay
    pub(crate) overlay_button_clicked: bool,

    // 左侧栏内联笔记编辑
    pub(crate) editing_note_sidebar_id: Option<String>,
    pub(crate) editing_note_sidebar_input: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) editing_note_sidebar_sub: Option<gpui::Subscription>,

    // 当前界面语言
    pub(crate) language: Language,

    // 搜索状态
    pub(crate) search_state: Option<SearchState>,
    pub(crate) search_input_state: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) search_input_sub: Option<gpui::Subscription>,
    pub(crate) search_list_state:
        Option<gpui::Entity<gpui_component::list::ListState<SearchResultsDelegate>>>,
    pub(crate) search_list_sub: Option<gpui::Subscription>,
    pub(crate) search_text_storage: Option<Vec<Option<TextPageData>>>,
    pub(crate) search_content_height: f32,

    // 文献笔记编辑
    pub(crate) notes_edit_mode: bool,
    pub(crate) notes_input_state: Option<gpui::Entity<gpui_component::input::InputState>>,
}

impl PdfReaderView {
    pub fn new(
        service: Arc<PdfService>,
        delegate: Option<Arc<dyn PdfReaderDelegate>>,
        document_id: String,
        _cx: &mut Context<Self>,
    ) -> Self {
        let list_state = ListState::new(0, ListAlignment::Top, px(1000.0));
        let thumbnail_list_state = ListState::new(0, ListAlignment::Top, px(600.0));

        let initial_state = if let Some(ref d) = delegate {
            d.get_initial_state(document_id.clone())
        } else {
            PdfInitialState::default()
        };

        let language = delegate
            .as_ref()
            .map(|d| d.current_language())
            .unwrap_or(Language::ZhCn);

        Self {
            pdf_service: service,
            delegate,
            document_id,
            current_page: initial_state.page_index,
            current_offset_y: initial_state.offset_y,
            total_pages: 0,
            page_sizes: Vec::new(),

            zoom_level: if initial_state.zoom_level > 0.1 {
                initial_state.zoom_level
            } else {
                1.0
            },
            render_zoom: if initial_state.zoom_level > 0.1 {
                quantize_render_zoom(initial_state.zoom_level)
            } else {
                1.0
            },
            list_state,
            render_generation: 0,
            worker_state: WorkerState::Loading,
            last_rem_size: 16.0,
            last_zoom_level: if initial_state.zoom_level > 0.1 {
                initial_state.zoom_level
            } else {
                1.0
            },
            fit_to_width_mode: initial_state.fit_to_width,
            initial_state: initial_state.clone(),
            is_restoring: true,

            language,

            page_cache: LruCache::new(NonZeroUsize::new(PAGE_CACHE_SIZE).unwrap()),
            stale_cache: LruCache::new(NonZeroUsize::new(PAGE_CACHE_SIZE).unwrap()),
            raw_page_cache: LruCache::new(NonZeroUsize::new(PAGE_CACHE_SIZE).unwrap()),
            text_cache: LruCache::new(NonZeroUsize::new(TEXT_CACHE_SIZE).unwrap()),
            link_cache: LruCache::new(NonZeroUsize::new(LINK_CACHE_SIZE).unwrap()),
            find_char_cache: HashMap::new(),

            is_mouse_down: false,
            mouse_down_pos: None,
            is_selecting: false,
            is_panning: false,
            offset_x: 0.0,
            selection_start: None,
            selection_end: None,
            selected_text: None,
            rect_in_progress: None,

            is_dragging_scrollbar: false,
            drag_offset: 0.0,
            is_dragging_thumbnail_scrollbar: false,
            thumbnail_drag_offset: 0.0,

            is_left_sidebar_open: initial_state.is_left_sidebar_open,
            active_left_sidebar_tab: LeftSidebarTab::Thumbnails,
            thumbnail_cache: LruCache::new(NonZeroUsize::new(THUMBNAIL_CACHE_SIZE).unwrap()),
            thumbnail_list_state,
            is_right_sidebar_open: initial_state.is_right_sidebar_open,
            active_right_sidebar_tab: RightSidebarTab::Translation,
            translation_result: None,
            is_engine_menu_open: false,
            translation_original_expanded: initial_state.translation_original_expanded,
            translation_font_size: initial_state.translation_font_size,
            auto_translate: initial_state.auto_translate,

            left_sidebar_width: px(if initial_state.left_sidebar_width > 0.0 {
                initial_state.left_sidebar_width
            } else {
                DEFAULT_SIDEBAR_WIDTH
            }
            .clamp(MIN_LEFT_SIDEBAR_WIDTH, MAX_LEFT_SIDEBAR_WIDTH)),
            right_sidebar_width: px(if initial_state.right_sidebar_width > 0.0 {
                initial_state.right_sidebar_width
            } else {
                DEFAULT_SIDEBAR_WIDTH
            }
            .clamp(MIN_RIGHT_SIDEBAR_WIDTH, MAX_RIGHT_SIDEBAR_WIDTH)),
            dragging_left_resizer: false,
            dragging_right_resizer: false,
            last_content_width: 0.0,
            programmatic_scroll: false,
            annotation_state: AnnotationState::default(),
            annotation_version: 0,
            last_composited_version: 0,

            focus_handle: _cx.focus_handle(),
            has_focused: false,

            outlines: None,
            expanded_outlines: std::collections::HashSet::new(),
            note_input_state: None,
            note_input_sub: None,
            overlay_button_clicked: false,

            editing_note_sidebar_id: None,
            editing_note_sidebar_input: None,
            editing_note_sidebar_sub: None,

            search_state: None,
            search_input_state: None,
            search_input_sub: None,
            search_list_state: None,
            search_list_sub: None,
            search_text_storage: None,
            search_content_height: 0.0,

            notes_edit_mode: false,
            notes_input_state: None,
        }
    }

    pub fn init_workers(
        &mut self,
        response_rx: std::sync::mpsc::Receiver<PdfResponse>,
        cx: &mut Context<Self>,
    ) {
        info!("PDF View: 启动工作线程响应监听...");
        let executor = cx.background_executor().clone();
        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let cx = cx.clone();
            let executor = executor.clone();
            async move {
                loop {
                    while let Ok(response) = response_rx.try_recv() {
                        let _ = cx.update(|cx| {
                            let _ = this.update(cx, |this, cx| match response {
                                PdfResponse::DocumentLoaded {
                                    doc_id,
                                    page_count,
                                    page_sizes,
                                } => {
                                    info!(
                                        "PDF View: 文档已加载, ID: {}, 共 {} 页",
                                        doc_id, page_count
                                    );
                                    this.pdf_service.set_doc_id(doc_id);
                                    this.total_pages = page_count;
                                    this.page_sizes = page_sizes;
                                    this.worker_state = WorkerState::Running;
                                    this.list_state.reset(page_count);
                                    this.thumbnail_list_state.reset(page_count);
                                    this.is_restoring = true;

                                    // 加载注释
                                    if let Some(delegate) = &this.delegate {
                                        let annotations =
                                            delegate.load_annotations(&this.document_id);
                                        let mut page_map: HashMap<u16, Vec<Annotation>> =
                                            HashMap::new();
                                        for ann in annotations {
                                            page_map.entry(ann.page).or_default().push(ann);
                                        }
                                        this.annotation_state.annotations = page_map;
                                    }

                                    cx.notify();
                                }
                                PdfResponse::PageRendered {
                                    page,
                                    generation,
                                    image,
                                } => {
                                    this.on_page_rendered(page, generation, image, cx);
                                }
                                PdfResponse::ThumbnailRendered {
                                    page,
                                    generation: _,
                                    image,
                                } => {
                                    this.on_thumbnail_rendered(page, image, cx);
                                }
                                PdfResponse::LinksExtracted {
                                    page,
                                    generation,
                                    data,
                                } => {
                                    if generation == this.render_generation {
                                        this.link_cache.put(page, data);
                                        cx.notify();
                                    }
                                }
                                PdfResponse::TextExtracted {
                                    page,
                                    generation,
                                    data,
                                } => {
                                    this.on_text_extracted(page, generation, data, cx);
                                }
                                PdfResponse::OutlineExtracted { outlines, .. } => {
                                    this.outlines =
                                        Some(translate_outlines(outlines, this.language));
                                    cx.notify();
                                }
                                PdfResponse::FatalError(e) => {
                                    error!("PDF View: 收到致命错误: {}", e);
                                    this.worker_state = WorkerState::Failed(e);
                                    this.is_restoring = false;
                                    cx.notify();
                                }
                            });
                        });
                    }
                    if let Err(std::sync::mpsc::TryRecvError::Disconnected) = response_rx.try_recv()
                    {
                        error!("PDF View: 工作线程通道断开");
                        break;
                    }
                    executor.timer(std::time::Duration::from_millis(16)).await;
                }
            }
        })
        .detach();
    }

    pub fn translate_text(&mut self, text: String, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }

        let formatted = clean_translation_text(&text);

        info!("PdfReaderView: 开始翻译文本, 长度={}", formatted.len());
        self.translation_result = Some(TranslationResult {
            original: formatted.clone(),
            translated: None,
            is_loading: true,
            error: None,
        });
        cx.notify();

        if let Some(delegate) = self.delegate.clone() {
            cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result: anyhow::Result<String> = delegate.translate(formatted).await;
                    let _ = this.update(&mut cx, |this, cx| {
                        if let Some(ref mut res) = this.translation_result {
                            match result {
                                Ok(translated) => {
                                    info!("PdfReaderView: 翻译完成, 长度={}", translated.len());
                                    res.translated = Some(translated);
                                    res.is_loading = false;
                                }
                                Err(e) => {
                                    error!("PdfReaderView: 翻译失败: {}", e);
                                    res.error = Some(e.to_string());
                                    res.is_loading = false;
                                }
                            }
                        }
                        cx.notify();
                    });
                }
            })
            .detach();
        }
    }

    pub fn change_translation_font_size(&mut self, delta: f32, cx: &mut Context<Self>) {
        self.translation_font_size = (self.translation_font_size + delta).clamp(8.0, 32.0);
        if let Some(delegate) = &self.delegate {
            delegate.set_translation_font_size(self.translation_font_size);
        }
        cx.notify();
    }

    fn apply_horizontal_scroll(&mut self, dx: f32, window: &Window) {
        if dx == 0.0 {
            return;
        }
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
    }

    fn render_left_resizer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top_0()
            .left(self.left_sidebar_width - px(3.0))
            .bottom_0()
            .w(px(6.0))
            .occlude()
            .cursor_col_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.dragging_left_resizer = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.dragging_left_resizer {
                        this.dragging_left_resizer = false;
                        this.save_current_state();
                        cx.notify();
                    }
                }),
            )
    }

    fn render_right_resizer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport_width = window.viewport_size().width;
        div()
            .absolute()
            .top_0()
            .left(viewport_width - self.right_sidebar_width - px(3.0))
            .bottom_0()
            .w(px(6.0))
            .occlude()
            .cursor_col_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.dragging_right_resizer = true;
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if this.dragging_right_resizer {
                        this.dragging_right_resizer = false;
                        this.save_current_state();
                        cx.notify();
                    }
                }),
            )
    }
}

impl Render for PdfReaderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(ref delegate) = self.delegate {
            self.language = delegate.current_language();
        }
        let current_rem_size = f32::from(window.rem_size());
        let current_viewport_width = f32::from(window.viewport_size().width);

        // 如果处于自适应模式且可用内容宽度发生变化，则重新计算缩放
        let mut current_content_width = current_viewport_width;
        if self.is_left_sidebar_open {
            current_content_width -= f32::from(self.left_sidebar_width);
        }
        if self.is_right_sidebar_open {
            current_content_width -= f32::from(self.right_sidebar_width);
        }

        if self.fit_to_width_mode && (current_content_width - self.last_content_width).abs() > 0.1 {
            self.last_content_width = current_content_width;
            self.apply_auto_fit(window, cx);
        }

        let get_page_height = |ix: usize, zoom: f32, rem_size: f32| -> f32 {
            let (pdf_w, pdf_h) = self.page_sizes.get(ix).copied().unwrap_or((612.0, 792.0));
            (PAGE_BASE_WIDTH_REMS * zoom * rem_size) * (pdf_h / pdf_w)
        };

        // 检测缩放或 DPI 变化，重置列表状态以重新计算高度
        if (self.zoom_level - self.last_zoom_level).abs() > 0.001
            || (current_rem_size - self.last_rem_size).abs() > 0.001
        {
            let saved_page = self.current_page;
            let saved_offset_y = self.current_offset_y;

            self.list_state.reset(self.total_pages);
            self.thumbnail_list_state.reset(self.total_pages);

            let px_offset = saved_offset_y * self.zoom_level * current_rem_size;
            self.list_state.scroll_to(ListOffset {
                item_ix: saved_page as usize,
                offset_in_item: px(px_offset),
            });

            self.thumbnail_list_state.scroll_to(ListOffset {
                item_ix: saved_page as usize,
                offset_in_item: px(0.0),
            });

            self.last_zoom_level = self.zoom_level;
            self.last_rem_size = current_rem_size;
        }

        // 执行初始进度恢复
        if self.is_restoring && self.total_pages > 0 {
            let page_index = self.initial_state.page_index as usize;
            if page_index < self.total_pages {
                let px_offset = self.initial_state.offset_y * self.zoom_level * self.last_rem_size;
                self.list_state.scroll_to(ListOffset {
                    item_ix: page_index,
                    offset_in_item: px(px_offset),
                });

                self.current_page = page_index as u16;
                self.current_offset_y = self.initial_state.offset_y;
                self.thumbnail_list_state.scroll_to(ListOffset {
                    item_ix: page_index,
                    offset_in_item: px(0.0),
                });
            }
            self.is_restoring = false;
        } else if self.total_pages > 0 {
            let scroll_top = self.list_state.logical_scroll_top();
            let toolbar_height = rems(TOOLBAR_HEIGHT_REMS).to_pixels(window.rem_size());
            let view_height = window.viewport_size().height - toolbar_height;
            self.search_content_height = f32::from(view_height);
            // 计算视窗顶部在全局坐标系中的绝对位置
            let mut viewport_top_abs = 0.0;
            for i in 0..scroll_top.item_ix {
                viewport_top_abs += get_page_height(i, self.zoom_level, self.last_rem_size);
            }
            viewport_top_abs += f32::from(scroll_top.offset_in_item);

            // 寻找顶部落在哪个页面
            let mut accumulated_height = 0.0;
            let mut new_page = 0;
            let mut new_offset_y = 0.0;

            for i in 0..self.total_pages {
                let page_h = get_page_height(i, self.zoom_level, self.last_rem_size);
                if accumulated_height + page_h > viewport_top_abs || i == self.total_pages - 1 {
                    new_page = i as u16;
                    let scaled_offset_to_top = viewport_top_abs - accumulated_height;
                    if self.zoom_level > 0.0 && self.last_rem_size > 0.0 {
                        new_offset_y =
                            scaled_offset_to_top / (self.zoom_level * self.last_rem_size);
                    }
                    break;
                }
                accumulated_height += page_h;
            }

            if !self.programmatic_scroll
                && (self.current_page != new_page
                    || (self.current_offset_y - new_offset_y).abs() > 0.01)
            {
                let page_changed = self.current_page != new_page;
                self.current_page = new_page;
                self.current_offset_y = new_offset_y;
                self.save_current_state();

                if page_changed && self.is_left_sidebar_open {
                    let thumbnail_scroll = self.thumbnail_list_state.logical_scroll_top();
                    // 如果当前页面已经对齐在顶部（容差 1px），则不触发强制对齐，防止拉动侧边栏时产生跳变
                    let already_aligned = thumbnail_scroll.item_ix == new_page as usize
                        && f32::from(thumbnail_scroll.offset_in_item).abs() < 1.0;

                    if !already_aligned {
                        let target_page = new_page as usize;
                        cx.on_next_frame(window, move |this, _win, cx| {
                            this.thumbnail_list_state.scroll_to_reveal_item(target_page);
                            cx.notify();
                        });
                    }
                }
            }
            self.programmatic_scroll = false;
        }

        if let WorkerState::Failed(ref msg) = self.worker_state {
            let msg = msg.clone();
            return div()
                .size_full()
                .bg(cx.theme().background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .gap_4()
                        .items_center()
                        .child(
                            Icon::new(PdfIconName::Close)
                                .size(px(48.0))
                                .text_color(gpui::red()),
                        )
                        .child(
                            Label::new(i18n::t(I18nKey::PdfEngineError, self.language))
                                .text_color(gpui::red()),
                        )
                        .child(
                            Label::new(msg)
                                .text_sm()
                                .text_color(gpui::red().opacity(0.7)),
                        )
                        .child(
                            Button::new("close_error")
                                .label(i18n::t(I18nKey::CloseWindow, self.language))
                                .on_click(|_, window: &mut Window, _| {
                                    window.remove_window();
                                }),
                        ),
                )
                .into_any_element();
        }

        if self.worker_state == WorkerState::Loading {
            return div()
                .size_full()
                .bg(cx.theme().background)
                .child(
                    h_flex()
                        .size_full()
                        .items_center()
                        .justify_center()
                        .child("Loading Document..."),
                )
                .into_any_element();
        }

        if !self.has_focused && self.worker_state == WorkerState::Running {
            self.has_focused = true;
            let handle = self.focus_handle.clone();
            window.on_next_frame(move |window, _| {
                window.focus(&handle);
            });
        }

        let theme = cx.theme();

        v_flex()
            .size_full()
            .relative()
            .bg(theme.background)
            .track_focus(&self.focus_handle)
            .on_mouse_move(
                cx.listener(|this, event, window, cx| {
                    this.handle_root_mouse_move(event, window, cx)
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                if event.keystroke.key.as_str() == "c"
                    && (event.keystroke.modifiers.control || event.keystroke.modifiers.platform)
                {
                    if let Some(ref text) = this.selected_text
                        && !text.is_empty()
                    {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.handle_root_mouse_up(cx);
                }),
            )
            .child(self.render_toolbar(window, cx))
            .child(
                h_flex()
                    .flex_grow()
                    .h_0()
                    .relative()
                    .when(self.is_left_sidebar_open, |this| {
                        this.child(self.render_left_sidebar(window, cx))
                    })
                    .when(self.is_left_sidebar_open, |this| {
                        this.child(self.render_left_resizer(cx))
                    })
                    .child(self.render_main_content(window, cx))
                    .when(self.is_right_sidebar_open, |this| {
                        this.child(self.render_right_resizer(window, cx))
                    })
                    .when(self.is_right_sidebar_open, |this| {
                        this.child(self.render_right_sidebar(window, cx))
                    })
                    .when_some(self.render_annotation_toolbar(window, cx), |this, tb| {
                        this.child(tb)
                    })
                    .when_some(
                        self.render_annotation_context_menu(window, cx),
                        |this, menu| this.child(menu),
                    )
                    .when_some(self.render_note_editor(window, cx), |this, editor| {
                        this.child(editor)
                    }),
            )
            .into_any_element()
    }
}

fn translate_outlines(items: Vec<crate::OutlineItem>, lang: Language) -> Vec<crate::OutlineItem> {
    let unnamed = i18n::t(I18nKey::UnnamedBookmark, lang);
    items
        .into_iter()
        .map(|mut item| {
            if item.title == "未命名书签" {
                item.title = unnamed.to_string();
            }
            item.children = translate_outlines(item.children, lang);
            item
        })
        .collect()
}

impl Drop for PdfReaderView {
    fn drop(&mut self) {
        info!("PdfReaderView: 视图销毁, 保存阅读状态");
        self.save_current_state();
    }
}

impl Focusable for PdfReaderView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
