use self::text_format::clean_translation_text;
use crate::{
    AiBackendItem, Annotation, AnnotationState, PdfInitialState, PdfReaderDelegate, PdfResponse,
    PdfService, TextPageData,
};
use ::components::{IconName, Side, render_resize_handle};
use gpui::prelude::*;
use gpui::{
    App, AsyncApp, ClipboardItem, Context, DragMoveEvent, Entity, FocusHandle, Focusable,
    KeyDownEvent, ListAlignment, ListOffset, ListState, MouseButton, MouseMoveEvent, MouseUpEvent,
    Pixels, Point, Render, WeakEntity, Window, deferred, div, px, rems,
};
use gpui_component::menu::PopupMenu;
use gpui_component::select::SelectEvent;
use gpui_component::{ActiveTheme, Icon, button::Button, h_flex, label::Label, v_flex};

use i18n::{I18nKey, Language};
use log::{debug, error, info};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod actions;
pub(crate) mod components;
pub(crate) mod helpers;
pub(crate) use components::pip;
mod selection;
mod text_format;
pub(crate) mod types;

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
    pub(crate) window_scale_factor: f32,
    pub(crate) last_render_scale_factor: f32,
    pub(crate) list_state: ListState,
    pub(crate) worker_state: WorkerState,
    pub(crate) initial_state: PdfInitialState,
    pub(crate) is_restoring: bool,
    pub(crate) last_rem_size: f32,
    pub(crate) last_zoom_level: f32,
    pub(crate) fit_to_width_mode: bool,

    // 页面数据（一次性加载所有页面）
    pub(crate) page_images: Vec<Option<gpui::ImageSource>>,
    pub(crate) raw_page_images: Vec<Option<Arc<image::RgbaImage>>>,
    pub(crate) page_text_data: Vec<Option<Arc<crate::TextPageData>>>,
    pub(crate) page_link_data: Vec<Option<Arc<crate::LinkPageData>>>,
    pub(crate) thumbnail_images: Vec<Option<gpui::ImageSource>>,
    pub(crate) pending_drop_images: Vec<Arc<gpui::RenderImage>>,
    /// 缩略图专用的文字数据（250px 分辨率下），延迟加载
    pub(crate) thumbnail_text_data: Vec<Option<Arc<crate::TextPageData>>>,
    /// 已发出缩略图文字请求的页面集合，用于去重
    pub(crate) thumbnail_text_requests_pending: HashSet<u16>,
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
    pub(crate) rect_start_pos: Option<(u16, f32, f32)>,            // (page_index, start_x, start_y)

    // 交互
    pub(crate) is_dragging_scrollbar: bool,
    pub(crate) drag_offset: f32,
    pub(crate) is_dragging_thumbnail_scrollbar: bool,
    pub(crate) thumbnail_drag_offset: f32,

    // 左侧边栏状态
    pub(crate) is_left_sidebar_open: bool,
    pub(crate) active_left_sidebar_tab: LeftSidebarTab,
    pub(crate) thumbnail_list_state: ListState,
    // 右侧边栏状态
    pub(crate) is_right_sidebar_open: bool,
    pub(crate) active_right_sidebar_tab: RightSidebarTab,
    pub(crate) translation_result: Option<TranslationResult>,
    pub(crate) engine_select:
        Option<gpui::Entity<gpui_component::select::SelectState<Vec<TranslationEngineItem>>>>,
    pub(crate) translation_original_expanded: bool,
    pub(crate) translation_font_size: f32,
    pub(crate) auto_translate: bool,

    pub(crate) document_id: String,
    // 侧边栏宽度与拖拽状态
    pub(crate) left_sidebar_width: gpui::Pixels,
    pub(crate) right_sidebar_width: gpui::Pixels,
    pub(crate) preferred_left_sidebar_width: f32,
    pub(crate) preferred_right_sidebar_width: f32,
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
    pub(crate) search_text_storage: Option<Vec<Option<Arc<TextPageData>>>>,
    pub(crate) search_content_height: f32,

    // 多笔记卡片
    pub(crate) notes_cache: Vec<models::LiteratureNote>,
    pub(crate) editing_note_index: Option<usize>,
    pub(crate) edit_note_title: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) edit_note_content: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) summary_task: Option<gpui::Task<()>>,
    pub(crate) is_generating_summary: bool,
    pub(crate) last_ai_summary_note_id: Option<String>,
    pub(crate) expanded_notes: std::collections::HashSet<String>,

    // ─── AI 对话 ─────────────────────────────────────────
    pub(crate) chat_sessions: Vec<models::chat::ChatSession>,
    pub(crate) active_chat_session_id: Option<String>,
    pub(crate) chat_creating: bool,
    pub(crate) chat_create_title: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) chat_create_prompt: Option<gpui::Entity<gpui_component::input::InputState>>,
    pub(crate) chat_session_view:
        Option<gpui::Entity<components::chat_session_view::ChatSessionView>>,
    pub(crate) chat_backend_select:
        Option<gpui::Entity<gpui_component::select::SelectState<Vec<AiBackendItem>>>>,

    // ─── 页面可见性管理 ─────────────────────────────────
    pub(crate) visible_page_first: usize,
    pub(crate) visible_page_last: usize,
    pub(crate) page_render_requests_pending: HashSet<u16>,
    // ─── 缩略图可见性管理 ──────────────────────────────
    pub(crate) visible_thumb_first: usize,
    pub(crate) visible_thumb_last: usize,
    pub(crate) thumb_render_requests_pending: HashSet<u16>,

    // ─── 画中画 (PiP) ────────────────────────────────────
    pub(crate) pins: Vec<pip::PiPPin>,
    #[allow(dead_code)]
    pub(crate) active_pin_id: Option<String>,
    pub(crate) dragging_pin: Option<pip::PiPDragState>,
    pub(crate) resizing_pin: Option<pip::PiPResizeState>,
    /// Pin 右键菜单：(菜单位置, PopupMenu 实体)
    pub(crate) pin_context_menu: Option<(gpui::Point<gpui::Pixels>, gpui::Entity<PopupMenu>)>,
    /// 注释右键菜单：(菜单位置, PopupMenu 实体)
    pub(crate) annotation_context_menu:
        Option<(gpui::Point<gpui::Pixels>, gpui::Entity<PopupMenu>)>,
    /// 缩略图右键菜单：(菜单位置, PopupMenu 实体)
    pub(crate) thumbnail_context_menu: Option<(gpui::Point<gpui::Pixels>, gpui::Entity<PopupMenu>)>,
    /// 浮动工具栏（选中文本后出现的颜色/类型选择菜单）
    pub(crate) annotation_toolbar_menu:
        Option<(gpui::Point<gpui::Pixels>, gpui::Entity<PopupMenu>)>,
    pub(crate) annotation_drag: Option<AnnotationDragState>,
    /// 当前文档标题（论文名），供保存图片等场景使用
    pub(crate) document_title: String,
    pub(crate) page_color_mode: PageColorMode,
    /// 缩放刚变化，需要以新分辨率重新渲染
    pub(crate) zoom_changed: bool,
    /// 主窗口 Tab 栏高度偏移（rems，嵌入时使用）
    pub(crate) tab_bar_offset_px: f32,
    /// 简单预览模式：隐藏工具栏
    pub(crate) hide_toolbar: bool,
    /// 简单预览模式：隐藏侧栏
    pub(crate) hide_sidebars: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TextStart,
    TextEnd,
}

#[derive(Clone, Debug)]
pub struct AnnotationDragState {
    pub annotation_id: String,
    pub page: u16,
    pub handle: AnnotationResizeHandle,
    pub start_mouse: gpui::Point<gpui::Pixels>,
    pub start_x: f32,
    pub start_y: f32,
    pub start_w: f32,
    pub start_h: f32,
}

impl PdfReaderView {
    pub fn drain_images_to_drop(&mut self) -> Vec<Arc<gpui::RenderImage>> {
        let mut images = Vec::new();
        // 收集并清除主页面纹理
        for opt in self.page_images.drain(..) {
            if let Some(gpui::ImageSource::Render(r)) = opt {
                images.push(r);
            }
        }
        self.raw_page_images.clear();

        // 收集并清除缩略图纹理
        for opt in self.thumbnail_images.drain(..) {
            if let Some(gpui::ImageSource::Render(r)) = opt {
                images.push(r);
            }
        }

        // 收集并清除 Pin 裁剪图纹理
        for mut pin in std::mem::take(&mut self.pins) {
            if let Some(gpui::ImageSource::Render(r)) = pin.image_source.take() {
                images.push(r);
            }
        }
        images
    }

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

        let global_ui = if _cx.has_global::<crate::GlobalPdfUiState>() {
            Some(_cx.global::<crate::GlobalPdfUiState>().clone())
        } else {
            None
        };

        let use_zoom = global_ui
            .as_ref()
            .map(|g| g.zoom_level)
            .unwrap_or(initial_state.zoom_level);
        let use_fit_to_width = global_ui
            .as_ref()
            .map(|g| g.fit_to_width)
            .unwrap_or(initial_state.fit_to_width);
        let use_is_left_sidebar_open = global_ui
            .as_ref()
            .map(|g| g.is_left_sidebar_open)
            .unwrap_or(initial_state.is_left_sidebar_open);
        let use_is_right_sidebar_open = global_ui
            .as_ref()
            .map(|g| g.is_right_sidebar_open)
            .unwrap_or(initial_state.is_right_sidebar_open);
        let use_left_sidebar_width = global_ui
            .as_ref()
            .map(|g| g.left_sidebar_width)
            .unwrap_or(initial_state.left_sidebar_width);
        let use_right_sidebar_width = global_ui
            .as_ref()
            .map(|g| g.right_sidebar_width)
            .unwrap_or(initial_state.right_sidebar_width);
        let use_auto_translate = global_ui
            .as_ref()
            .map(|g| g.auto_translate)
            .unwrap_or(initial_state.auto_translate);

        let language = delegate
            .as_ref()
            .map(|d| d.current_language())
            .unwrap_or(Language::ZhCn);

        let initial_page_color_mode = if let Some(d) = &delegate {
            match d.get_page_color_mode().as_str() {
                "sepia" => PageColorMode::Sepia,
                "eyeprotect" => PageColorMode::EyeProtect,
                _ => PageColorMode::White,
            }
        } else {
            PageColorMode::White
        };

        let view_instance = Self {
            pdf_service: service,
            delegate,
            document_id: document_id.clone(),
            document_title: document_id,
            current_page: initial_state.page_index,
            current_offset_y: initial_state.offset_y,
            total_pages: 0,
            page_sizes: Vec::new(),

            zoom_level: if use_zoom > 0.1 { use_zoom } else { 1.0 },
            render_zoom: if use_zoom > 0.1 {
                quantize_render_zoom(use_zoom)
            } else {
                1.0
            },
            list_state,
            worker_state: WorkerState::Loading,
            last_rem_size: 16.0,
            window_scale_factor: 1.0,
            last_render_scale_factor: 0.0,
            last_zoom_level: if use_zoom > 0.1 { use_zoom } else { 1.0 },
            fit_to_width_mode: use_fit_to_width,
            initial_state: initial_state.clone(),
            is_restoring: true,

            language,

            // 页面数据初始化（文档加载后会重置）
            page_images: Vec::new(),
            raw_page_images: Vec::new(),
            page_text_data: Vec::new(),
            page_link_data: Vec::new(),
            thumbnail_images: Vec::new(),
            pending_drop_images: Vec::new(),
            thumbnail_text_data: Vec::new(),
            thumbnail_text_requests_pending: HashSet::new(),

            // 页面可见性管理
            visible_page_first: 0,
            visible_page_last: 0,
            page_render_requests_pending: HashSet::new(),
            // 缩略图可见性管理
            visible_thumb_first: 0,
            visible_thumb_last: 0,
            thumb_render_requests_pending: HashSet::new(),
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
            rect_start_pos: None,

            is_dragging_scrollbar: false,
            drag_offset: 0.0,
            is_dragging_thumbnail_scrollbar: false,
            thumbnail_drag_offset: 0.0,

            is_left_sidebar_open: use_is_left_sidebar_open,
            active_left_sidebar_tab: LeftSidebarTab::Thumbnails,
            thumbnail_list_state,
            is_right_sidebar_open: use_is_right_sidebar_open,
            active_right_sidebar_tab: RightSidebarTab::Translation,
            translation_result: None,
            engine_select: None,
            translation_original_expanded: initial_state.translation_original_expanded,
            translation_font_size: initial_state.translation_font_size,
            auto_translate: use_auto_translate,

            preferred_left_sidebar_width: if use_left_sidebar_width > 0.0 {
                use_left_sidebar_width
            } else {
                DEFAULT_LEFT_SIDEBAR_WIDTH
            },
            preferred_right_sidebar_width: if use_right_sidebar_width > 0.0 {
                use_right_sidebar_width
            } else {
                DEFAULT_RIGHT_SIDEBAR_WIDTH
            },
            left_sidebar_width: px(if use_left_sidebar_width > 0.0 {
                use_left_sidebar_width
            } else {
                DEFAULT_LEFT_SIDEBAR_WIDTH
            }),
            right_sidebar_width: px(if use_right_sidebar_width > 0.0 {
                use_right_sidebar_width
            } else {
                DEFAULT_RIGHT_SIDEBAR_WIDTH
            }),
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

            notes_cache: Vec::new(),
            editing_note_index: None,
            edit_note_title: None,
            edit_note_content: None,
            summary_task: None,
            is_generating_summary: false,
            last_ai_summary_note_id: None,
            expanded_notes: std::collections::HashSet::new(),

            chat_sessions: Vec::new(),
            active_chat_session_id: None,
            chat_creating: false,
            chat_create_title: None,
            chat_create_prompt: None,
            chat_session_view: None,
            chat_backend_select: None,

            pins: Vec::new(),
            active_pin_id: None,
            dragging_pin: None,
            resizing_pin: None,
            pin_context_menu: None,
            annotation_context_menu: None,
            thumbnail_context_menu: None,
            annotation_toolbar_menu: None,
            annotation_drag: None,
            page_color_mode: initial_page_color_mode,
            zoom_changed: false,
            tab_bar_offset_px: 0.0,
            hide_toolbar: false,
            hide_sidebars: false,
        };

        if !_cx.has_global::<crate::GlobalPdfUiState>() {
            _cx.set_global(crate::GlobalPdfUiState {
                zoom_level: use_zoom,
                fit_to_width: use_fit_to_width,
                is_left_sidebar_open: use_is_left_sidebar_open,
                is_right_sidebar_open: use_is_right_sidebar_open,
                left_sidebar_width: use_left_sidebar_width,
                right_sidebar_width: use_right_sidebar_width,
                auto_translate: use_auto_translate,
            });
        }

        _cx.observe_global::<crate::GlobalPdfUiState>(|this, cx| {
            let global = cx.global::<crate::GlobalPdfUiState>();
            let mut changed = false;

            if (this.zoom_level - global.zoom_level).abs() > 0.001 {
                this.zoom_level = global.zoom_level;
                let new_render_zoom = quantize_render_zoom(this.zoom_level);
                if (this.render_zoom - new_render_zoom).abs() > f32::EPSILON {
                    this.render_zoom = new_render_zoom;
                    this.find_char_cache.clear();
                    this.page_render_requests_pending.clear();
                    this.zoom_changed = true;
                }
                this.search_state = None;
                this.programmatic_scroll = true;
                changed = true;
            }
            if this.fit_to_width_mode != global.fit_to_width {
                this.fit_to_width_mode = global.fit_to_width;
                changed = true;
            }
            if this.is_left_sidebar_open != global.is_left_sidebar_open {
                this.is_left_sidebar_open = global.is_left_sidebar_open;
                changed = true;
            }
            if this.is_right_sidebar_open != global.is_right_sidebar_open {
                this.is_right_sidebar_open = global.is_right_sidebar_open;
                changed = true;
            }
            if (f32::from(this.left_sidebar_width) - global.left_sidebar_width).abs() > 0.1 {
                this.left_sidebar_width = px(global.left_sidebar_width);
                this.preferred_left_sidebar_width = global.left_sidebar_width;
                changed = true;
            }
            if (f32::from(this.right_sidebar_width) - global.right_sidebar_width).abs() > 0.1 {
                this.right_sidebar_width = px(global.right_sidebar_width);
                this.preferred_right_sidebar_width = global.right_sidebar_width;
                changed = true;
            }
            if this.auto_translate != global.auto_translate {
                this.auto_translate = global.auto_translate;
                changed = true;
            }

            if changed {
                cx.notify();
            }
        })
        .detach();

        view_instance
    }

    pub fn set_tab_bar_offset_px(&mut self, px: f32) {
        self.tab_bar_offset_px = px;
    }

    pub fn set_document_title(&mut self, title: String) {
        self.document_title = title;
    }

    pub fn set_simple_mode(&mut self, simple: bool) {
        self.hide_toolbar = simple;
        self.hide_sidebars = simple;
        self.is_left_sidebar_open = false;
        self.is_right_sidebar_open = false;
    }

    pub(crate) fn set_page_color_mode(&mut self, mode: PageColorMode, cx: &mut Context<Self>) {
        if self.page_color_mode == mode {
            return;
        }
        self.page_color_mode = mode;

        if let Some(d) = &self.delegate {
            let mode_str = match self.page_color_mode {
                PageColorMode::White => "white",
                PageColorMode::Sepia => "sepia",
                PageColorMode::EyeProtect => "eyeprotect",
            };
            d.set_page_color_mode(mode_str.to_string());
        }

        // 背景色通过 div bg() 自动生效，无需清缓存或重渲染
        cx.notify();
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
                    let mut disconnected = false;
                    loop {
                        match response_rx.try_recv() {
                            Ok(response) => {
                                cx.update(|cx| {
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
                                            this.total_pages = page_count;
                                            this.page_sizes = page_sizes;
                                            this.worker_state = WorkerState::Running;
                                            this.list_state.reset(page_count);
                                            this.thumbnail_list_state.reset(page_count);
                                            this.is_restoring = true;

                                            // 初始化页面数据 Vec
                                            this.page_images = vec![None; page_count];
                                            this.raw_page_images = vec![None; page_count];
                                            this.page_text_data = vec![None; page_count];
                                            this.page_link_data = vec![None; page_count];
                                            this.thumbnail_images = vec![None; page_count];
                                            this.thumbnail_text_data = vec![None; page_count];
                                            this.thumbnail_text_requests_pending.clear();
                                            this.visible_page_first = usize::MAX;
                                            this.visible_page_last = 0;
                                            this.page_render_requests_pending.clear();
                                            this.visible_thumb_first = usize::MAX;
                                            this.visible_thumb_last = 0;
                                            this.thumb_render_requests_pending.clear();

                                            // 主页面和缩略图渲染由 render() 里的
                                            // refresh_page_visibility / refresh_thumb_visibility 触发
                                            // （DocumentLoaded 时 list_state 已重置，第一帧 render 会自动调度）

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
                                            generation: _,
                                            image,
                                        } => {
                                            this.on_page_rendered(page, image, cx);
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
                                            generation: _,
                                            data,
                                        } => {
                                            if let Some(slot) =
                                                this.page_link_data.get_mut(page as usize)
                                            {
                                                *slot = Some(Arc::new(data));
                                            }
                                            cx.notify();
                                        }
                                        PdfResponse::TextExtracted {
                                            page,
                                            generation,
                                            data,
                                        } => {
                                            if generation == 1 {
                                                // 缩略图文字：存入专用存储，不触发搜索
                                                this.thumbnail_text_requests_pending.remove(&page);
                                                if let Some(slot) =
                                                    this.thumbnail_text_data.get_mut(page as usize)
                                                {
                                                    *slot = Some(Arc::new(data));
                                                }
                                                cx.notify();
                                            } else {
                                                this.on_text_extracted(page, data, cx);
                                            }
                                        }
                                        PdfResponse::PinRendered { pin_id, image } => {
                                            debug!(
                                                "mod: 收到 Pin 渲染结果 pin_id={}, 分辨率 {}x{}",
                                                pin_id,
                                                image.width(),
                                                image.height()
                                            );
                                            if let Some(pin) =
                                                this.pins.iter_mut().find(|p| p.id == pin_id)
                                            {
                                                pin.raw_image = Some(Arc::new(image.clone()));
                                                pin.image_source =
                                                    Some(helpers::make_image_source(image));
                                                cx.notify();
                                            } else {
                                                debug!(
                                                    "mod: PinRendered 但 pin_id={} 已不存在",
                                                    pin_id
                                                );
                                            }
                                        }
                                        PdfResponse::OutlineExtracted { outlines, .. } => {
                                            this.outlines =
                                                Some(translate_outlines(outlines, this.language));
                                            cx.notify();
                                        }
                                        PdfResponse::DocumentModified {
                                            doc_id: _,
                                            page_count,
                                            page_sizes,
                                            deleted_page,
                                        } => {
                                            log::info!(
                                                "PDF View: 文档已修改, 共 {} 页",
                                                page_count
                                            );
                                            this.total_pages = page_count;
                                            this.page_sizes = page_sizes;
                                            this.list_state.reset(page_count);
                                            this.thumbnail_list_state.reset(page_count);

                                            let deleted_idx = deleted_page as usize;
                                            if deleted_idx < this.page_images.len() {
                                                this.page_images.remove(deleted_idx);
                                                this.raw_page_images.remove(deleted_idx);
                                                this.page_text_data.remove(deleted_idx);
                                                this.page_link_data.remove(deleted_idx);
                                                this.thumbnail_images.remove(deleted_idx);
                                                this.thumbnail_text_data.remove(deleted_idx);
                                            }
                                            this.thumbnail_text_requests_pending.clear();

                                            // 强制在下一帧进行可见性重新判定与渲染
                                            this.visible_page_first = usize::MAX;
                                            this.visible_page_last = 0;
                                            this.page_render_requests_pending.clear();
                                            this.visible_thumb_first = usize::MAX;
                                            this.visible_thumb_last = 0;
                                            this.thumb_render_requests_pending.clear();

                                            // 重新定位滚动位置并安全限制页码
                                            let target_page = (this.current_page as usize)
                                                .min(page_count.saturating_sub(1));
                                            this.list_state.scroll_to(gpui::ListOffset {
                                                item_ix: target_page,
                                                offset_in_item: gpui::px(0.0),
                                            });
                                            this.thumbnail_list_state.scroll_to(gpui::ListOffset {
                                                item_ix: target_page,
                                                offset_in_item: gpui::px(0.0),
                                            });
                                            this.current_page = target_page as u16;
                                            this.current_offset_y = 0.0;

                                            // 批注物理平移
                                            this.annotation_state.annotations.remove(&deleted_page);
                                            let mut new_annotations =
                                                std::collections::HashMap::new();
                                            for (page, mut anns) in
                                                this.annotation_state.annotations.drain()
                                            {
                                                if page < deleted_page {
                                                    new_annotations.insert(page, anns);
                                                } else if page > deleted_page {
                                                    for ann in &mut anns {
                                                        ann.page = page - 1;
                                                    }
                                                    new_annotations.insert(page - 1, anns);
                                                }
                                            }
                                            this.annotation_state.annotations = new_annotations;

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
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                break;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                disconnected = true;
                                break;
                            }
                        }
                    }
                    if disconnected {
                        error!("PDF View: 工作线程通道断开");
                        break;
                    }
                    executor.timer(std::time::Duration::from_millis(16)).await;
                }
            }
        })
        .detach();
    }

    pub fn translate_text(&mut self, text: String, force: bool, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }

        let formatted = clean_translation_text(&text);

        info!(
            "PdfReaderView: 开始翻译文本, 强制={}, 长度={}",
            force,
            formatted.len()
        );
        self.translation_result = Some(TranslationResult {
            original: formatted.clone(),
            translated: None,
            is_loading: true,
            error: None,
        });
        cx.notify();

        if let Some(delegate) = self.delegate.clone() {
            cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                async move {
                    let result: anyhow::Result<String> = delegate.translate(formatted, force).await;
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

    /// 从 ConfigStore observer 更新语言（观察者模式入口）
    pub fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.language = language;
        // 如果 SelectState 已存在，需要重新生成或更新其选项的语言（为了简单，我们清空它，下次获取时会重新用新语言创建）
        self.engine_select = None;
        cx.notify();
    }

    pub(crate) fn get_or_create_engine_select(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<gpui_component::select::SelectState<Vec<TranslationEngineItem>>> {
        let current_engine = self
            .delegate
            .as_ref()
            .map(|d| d.current_translation_engine_id())
            .unwrap_or_default();

        if let Some(select) = &self.engine_select {
            select.update(cx, |state, cx| {
                if state.selected_value() != Some(&current_engine) {
                    state.set_selected_value(&current_engine, window, cx);
                }
            });
            return select.clone();
        }

        let engines = self
            .delegate
            .as_ref()
            .map(|d| d.get_translation_engines())
            .unwrap_or_default();

        let engine_items: Vec<TranslationEngineItem> = engines
            .into_iter()
            .map(|id| {
                let label = match id.as_str() {
                    "google_free" => i18n::t(I18nKey::EngineGoogleFree, self.language).to_string(),
                    "bing_free" => i18n::t(I18nKey::EngineBingFree, self.language).to_string(),
                    "google" => i18n::t(I18nKey::EngineGoogleCloud, self.language).to_string(),
                    "niutrans" => i18n::t(I18nKey::EngineNiuTrans, self.language).to_string(),
                    "baidu" => i18n::t(I18nKey::EngineBaidu, self.language).to_string(),
                    "youdao" => i18n::t(I18nKey::EngineYoudao, self.language).to_string(),
                    "deepl_free" => i18n::t(I18nKey::EngineDeeplFree, self.language).to_string(),
                    "deepl_pro" => i18n::t(I18nKey::EngineDeeplPro, self.language).to_string(),
                    "ai" => i18n::t(I18nKey::EngineAi, self.language).to_string(),
                    _ => id.clone(),
                };
                TranslationEngineItem { value: id, label }
            })
            .collect();

        let select = cx.new(|cx| {
            let mut state =
                gpui_component::select::SelectState::new(engine_items, None, window, cx);
            state.set_selected_value(&current_engine, window, cx);
            state
        });

        cx.subscribe(&select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(engine_id)) = event
                && let Some(delegate) = &this.delegate
            {
                delegate.set_translation_engine(engine_id.clone(), cx);
            }
        })
        .detach();

        self.engine_select = Some(select.clone());
        select
    }

    pub(crate) fn get_or_create_chat_backend_select(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<gpui_component::select::SelectState<Vec<AiBackendItem>>> {
        let current_name = self
            .delegate
            .as_ref()
            .and_then(|d| d.get_active_chat_backend());

        if let Some(select) = &self.chat_backend_select {
            select.update(cx, |state, cx| {
                if state.selected_value() != current_name.as_ref()
                    && let Some(ref name) = current_name
                {
                    state.set_selected_value(name, window, cx);
                }
            });
            return select.clone();
        }

        let items = self
            .delegate
            .as_ref()
            .map(|d| d.list_ai_backends())
            .unwrap_or_default();

        let select = cx.new(|cx| {
            let mut state = gpui_component::select::SelectState::new(items, None, window, cx);
            if let Some(ref name) = current_name {
                state.set_selected_value(name, window, cx);
            }
            state
        });

        cx.subscribe(&select, |this, _, event, _cx| {
            if let SelectEvent::Confirm(Some(name)) = event
                && let Some(delegate) = &this.delegate
            {
                delegate.set_active_chat_backend(name);
            }
        })
        .detach();

        self.chat_backend_select = Some(select.clone());
        select
    }

    pub fn delegate(&self) -> Option<&Arc<dyn PdfReaderDelegate>> {
        self.delegate.as_ref()
    }

    pub fn document_id(&self) -> &str {
        &self.document_id
    }

    pub fn set_notes_cache(&mut self, notes: Vec<models::LiteratureNote>) {
        self.notes_cache = notes;
    }

    pub fn reload_notes(&mut self, cx: &mut Context<Self>) {
        if let Some(delegate) = &self.delegate {
            let lit_id = self
                .document_id
                .split("::")
                .next()
                .unwrap_or(&self.document_id);
            let notes = delegate.list_notes(lit_id);
            let has_generating = self.is_generating_summary;
            let mut merged_notes = notes;
            if has_generating
                && let Some(gen_note) = self
                    .notes_cache
                    .iter()
                    .find(|n| n.id == "ai_generating_note")
                    .cloned()
            {
                merged_notes.push(gen_note);
            }
            self.notes_cache = merged_notes;
        }
        cx.notify();
    }

    pub fn reload_chat_sessions(&mut self, cx: &mut Context<Self>) {
        if let Some(delegate) = &self.delegate {
            let lit_id = self
                .document_id
                .split("::")
                .next()
                .unwrap_or(&self.document_id);
            self.chat_sessions = delegate.list_chat_sessions(lit_id);
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

    fn render_sidebar_resizer(&self, is_left: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let (side, offset) = if is_left {
            (Side::Left, self.left_sidebar_width)
        } else {
            (Side::Right, self.right_sidebar_width)
        };

        let line = div()
            .absolute()
            .top_0()
            .h_full()
            .w(rems(0.125))
            .bg(cx.theme().border);

        let line = match side {
            Side::Left => line.left(offset - px(1.0)),
            Side::Right => line.right(offset - px(1.0)),
        };

        let hot_zone = render_resize_handle(side, offset)
            .id(if is_left {
                "pdf-left-resizer"
            } else {
                "pdf-right-resizer"
            })
            .on_drag(DraggedSidebar(is_left), |drag, _, _, cx| {
                cx.new(|_| drag.clone())
            });

        div()
            .absolute()
            .top_0()
            .left_0()
            .w(px(0.0))
            .h_full()
            .child(line)
            .child(deferred(hot_zone))
    }

    pub fn is_content_interacting(&self) -> bool {
        self.is_mouse_down
            || self.annotation_drag.is_some()
            || self.dragging_pin.is_some()
            || self.resizing_pin.is_some()
    }

    pub fn handle_global_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_content_interacting() {
            self.handle_content_mouse_move(event, window, cx);
        } else if self.is_dragging_scrollbar || self.is_dragging_thumbnail_scrollbar {
            self.handle_root_mouse_move(event, window, cx);
        }
    }

    pub fn handle_global_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_content_interacting() {
            self.handle_content_mouse_up(event.position, window, cx);
            self.is_panning = false;
        } else if self.is_dragging_scrollbar || self.is_dragging_thumbnail_scrollbar {
            self.handle_root_mouse_up(cx);
        }
    }

    fn render_menu_overlay(
        &self,
        pos: Point<Pixels>,
        menu: Entity<PopupMenu>,
        window: &Window,
        menu_w: f32,
        menu_h: f32,
    ) -> impl IntoElement {
        let local_x = f32::from(pos.x).max(0.0);
        let local_y = f32::from(pos.y);
        let h_flex_w = f32::from(window.viewport_size().width);
        let h_flex_h = f32::from(window.viewport_size().height) - self.tab_bar_offset_px;

        let clamp_x = local_x.clamp(0.0, (h_flex_w - menu_w).max(0.0));
        let clamp_y = local_y.min((h_flex_h - menu_h).max(0.0));

        div()
            .absolute()
            .left(px(clamp_x))
            .top(px(clamp_y))
            .cursor_default()
            .child(menu)
    }
}

impl Render for PdfReaderView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 在绘图帧开头，物理释放所有由于图片覆盖等延迟的 GPU 纹理资源
        if !self.pending_drop_images.is_empty() {
            let count = self.pending_drop_images.len();
            for img in self.pending_drop_images.drain(..) {
                if let Err(e) = window.drop_image(img) {
                    log::error!("drop_image failed: {e}");
                }
            }
            debug!(
                "PdfReaderView: 延迟释放队列处理完成，释放了 {} 张覆盖纹理",
                count
            );
        }

        let viewport_width = window.viewport_size().width;
        if viewport_width > px(0.0) {
            self.left_sidebar_width = px(self.preferred_left_sidebar_width).clamp(
                px(f32::from(viewport_width) * SIDEBAR_MIN_RATIO),
                px(f32::from(viewport_width) * SIDEBAR_MAX_RATIO),
            );
            self.right_sidebar_width = px(self.preferred_right_sidebar_width).clamp(
                px(f32::from(viewport_width) * SIDEBAR_MIN_RATIO),
                px(f32::from(viewport_width) * SIDEBAR_MAX_RATIO),
            );
        } else {
            self.left_sidebar_width = px(self.preferred_left_sidebar_width);
            self.right_sidebar_width = px(self.preferred_right_sidebar_width);
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

        if self.fit_to_width_mode && (current_content_width - self.last_content_width).abs() > 1.0 {
            self.last_content_width = current_content_width;
            self.apply_auto_fit(window, cx);
        }

        let get_page_height = |ix: usize, zoom: f32, rem_size: f32| {
            helpers::page_height(&self.page_sizes, ix, zoom, rem_size)
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
            let tab_bar_h = self.tab_bar_offset_px;
            let view_height =
                f32::from(window.viewport_size().height) - tab_bar_h - f32::from(toolbar_height);
            self.search_content_height = view_height;
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
                self.save_current_state(Some(cx));

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

        // 页面可见性管理：计算视口范围、淘汰远页、调度渲染
        if self.worker_state == WorkerState::Running {
            // 先读取 window scale_factor / rem_size，确保 refresh 中使用的缩放比例正确
            self.window_scale_factor = window.scale_factor();
            self.last_rem_size = f32::from(window.rem_size());
            self.refresh_page_visibility(window, cx);
            if self.is_left_sidebar_open {
                self.refresh_thumb_visibility(window, cx);
            }
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
                            Icon::new(IconName::Close)
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
            window.on_next_frame(move |window, cx| {
                window.focus(&handle, cx);
            });
        }

        // 如果 scale_factor 发生变化，清除缓存让 refresh_page_visibility 重新调度
        let scale_factor = window.scale_factor();
        if self.worker_state == WorkerState::Running
            && (self.last_render_scale_factor - scale_factor).abs() > 0.01
        {
            self.last_render_scale_factor = scale_factor;
            debug!(
                "mod: 窗口 scale_factor 变更为 {}, 等待 refresh_page_visibility 重新调度",
                scale_factor
            );
            // 清空页面缓存，触发重新渲染
            for img in self.page_images.iter_mut() {
                *img = None;
            }
            for img in self.raw_page_images.iter_mut() {
                *img = None;
            }
            self.page_render_requests_pending.clear();
            // 强制下一帧 refresh_page_visibility 进入调度路径
            self.visible_page_first = usize::MAX;
            self.visible_page_last = 0;
        }

        // 惰性构建浮动工具栏 PopupMenu（需要 &mut Window，在 theme 之前）
        if self.annotation_state.toolbar.is_some() && self.annotation_toolbar_menu.is_none() {
            self.annotation_toolbar_menu = self.build_toolbar_popup_menu(window, cx);
        }
        // 每帧刷新位置（跟随滚动 + 边界避碰）
        if self.annotation_toolbar_menu.is_some()
            && let Some((x, y)) = self.compute_toolbar_screen_pos(window)
        {
            self.annotation_toolbar_menu.as_mut().unwrap().0 = gpui::Point { x, y };
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
            .on_drag_move::<DraggedSidebar>(cx.listener(
                |this, event: &DragMoveEvent<DraggedSidebar>, window, cx| {
                    let viewport_width = window.viewport_size().width;
                    let min_width = px(f32::from(viewport_width) * SIDEBAR_MIN_RATIO);
                    let max_width = px(f32::from(viewport_width) * SIDEBAR_MAX_RATIO);

                    if event.drag(cx).0 {
                        // 左侧 resizer
                        let current_right_w = if this.is_right_sidebar_open {
                            this.right_sidebar_width
                        } else {
                            px(0.0)
                        };
                        let available_for_left =
                            (viewport_width - current_right_w - px(300.0)).max(min_width);
                        let final_max = max_width.min(available_for_left);

                        this.left_sidebar_width =
                            event.event.position.x.max(min_width).min(final_max);
                        this.preferred_left_sidebar_width = f32::from(this.left_sidebar_width);
                    } else {
                        // 右侧 resizer
                        let current_left_w = if this.is_left_sidebar_open {
                            this.left_sidebar_width
                        } else {
                            px(0.0)
                        };
                        let available_for_right =
                            (viewport_width - current_left_w - px(300.0)).max(min_width);
                        let final_max = max_width.min(available_for_right);

                        this.right_sidebar_width = (viewport_width - event.event.position.x)
                            .max(min_width)
                            .min(final_max);
                        this.preferred_right_sidebar_width = f32::from(this.right_sidebar_width);
                    }
                    this.save_current_state(Some(cx));
                    cx.notify();
                },
            ))
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
            .child(
                h_flex()
                    .flex_grow(1.0)
                    .h_0()
                    .relative()
                    .when(self.is_left_sidebar_open && !self.hide_sidebars, |this| {
                        this.child(self.render_left_sidebar(window, cx))
                    })
                    .child(
                        v_flex()
                            .flex_grow(1.0)
                            .h_full()
                            .when(!self.hide_toolbar, |this| {
                                this.child(self.render_toolbar(window, cx))
                            })
                            .child(
                                div()
                                    .flex_grow(1.0)
                                    .h_0()
                                    .w_full()
                                    .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                                        this.annotation_context_menu = None;
                                        this.pin_context_menu = None;
                                        this.thumbnail_context_menu = None;
                                        this.annotation_toolbar_menu = None;
                                        this.annotation_state.toolbar = None;
                                        this.selection_start = None;
                                        this.selection_end = None;
                                        this.selected_text = None;
                                        this.annotation_state.note_editor = None;
                                        this.note_input_state = None;
                                        this.note_input_sub = None;
                                        cx.notify();
                                    }))
                                    .child(self.render_main_content(window, cx)),
                            ),
                    )
                    .when(self.is_right_sidebar_open && !self.hide_sidebars, |this| {
                        this.child(self.render_right_sidebar(window, cx))
                    })
                    .when(self.is_left_sidebar_open && !self.hide_sidebars, |this| {
                        this.child(self.render_sidebar_resizer(true, cx))
                    })
                    .when(self.is_right_sidebar_open && !self.hide_sidebars, |this| {
                        this.child(self.render_sidebar_resizer(false, cx))
                    })
                    // 不再使用遮罩层：改为在阅读区容器上挂 capture_any_mouse_down 捕获阶段 handler
                    .when_some(
                        self.annotation_toolbar_menu.as_ref(),
                        |this, (pos, menu)| {
                            this.child(self.render_menu_overlay(
                                *pos,
                                menu.clone(),
                                window,
                                200.0,
                                80.0,
                            ))
                        },
                    )
                    .when_some(
                        self.annotation_context_menu.as_ref(),
                        |this, (pos, menu)| {
                            this.child(self.render_menu_overlay(
                                *pos,
                                menu.clone(),
                                window,
                                180.0,
                                220.0,
                            ))
                        },
                    )
                    .when_some(self.pin_context_menu.as_ref(), |this, (pos, menu)| {
                        this.child(self.render_menu_overlay(
                            *pos,
                            menu.clone(),
                            window,
                            180.0,
                            160.0,
                        ))
                    })
                    .when_some(self.thumbnail_context_menu.as_ref(), |this, (pos, menu)| {
                        this.child(self.render_menu_overlay(
                            *pos,
                            menu.clone(),
                            window,
                            180.0,
                            40.0,
                        ))
                    })
                    .when_some(self.render_note_editor(window, cx), |this, editor| {
                        this.child(editor)
                    }),
            )
            .when(
                self.dragging_pin.is_some() || self.resizing_pin.is_some(),
                |this| {
                    this.child(
                        div()
                            .absolute()
                            .inset_0()
                            .cursor_default()
                            .on_mouse_move(cx.listener(|this, event, window, cx| {
                                this.handle_pin_mouse_move(event, window, cx);
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.dragging_pin = None;
                                    this.resizing_pin = None;
                                    cx.notify();
                                }),
                            ),
                    )
                },
            )
            .when(
                self.is_dragging_scrollbar
                    || self.is_dragging_thumbnail_scrollbar
                    || self.is_panning,
                |this| {
                    this.child(
                        div()
                            .absolute()
                            .inset_0()
                            .cursor_default()
                            .on_mouse_move(cx.listener(|this, event, window, cx| {
                                this.handle_root_mouse_move(event, window, cx);
                            }))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.handle_root_mouse_up(cx);
                                }),
                            ),
                    )
                },
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
        self.save_current_state(None);
    }
}

impl Focusable for PdfReaderView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[derive(Clone)]
pub struct DraggedSidebar(pub bool); // true if left

impl Render for DraggedSidebar {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}
