use self::text_format::clean_translation_text;
use crate::{
    AiBackendItem, Annotation, AnnotationState, PdfInitialState, PdfReaderDelegate, PdfResponse,
    PdfService, TextPageData,
};
use gpui::prelude::*;
use gpui::{
    App, AsyncApp, ClipboardItem, Context, FocusHandle, Focusable, KeyDownEvent, ListAlignment,
    ListOffset, ListState, MouseButton, Render, WeakEntity, Window, div, px, rems,
};
use gpui_component::select::SelectEvent;
use gpui_component::{ActiveTheme, Icon, button::Button, h_flex, label::Label, v_flex};

use i18n::{I18nKey, Language};
use log::{error, info};
use std::collections::HashMap;
use std::sync::Arc;

pub mod actions;
pub mod components;
pub mod pip;
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

    // ─── 画中画 (PiP) ────────────────────────────────────
    pub(crate) pins: Vec<pip::PiPPin>,
    #[allow(dead_code)]
    pub(crate) active_pin_id: Option<String>,
    pub(crate) dragging_pin: Option<pip::PiPDragState>,
    pub(crate) resizing_pin: Option<pip::PiPResizeState>,
    pub(crate) annotation_drag: Option<AnnotationDragState>,
    pub(crate) page_color_mode: PageColorMode,
    /// 主窗口 Tab 栏高度偏移（rems，嵌入时使用）
    pub(crate) tab_bar_offset_rems: f32,
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

        let initial_page_color_mode = if let Some(d) = &delegate {
            match d.get_page_color_mode().as_str() {
                "sepia" => PageColorMode::Sepia,
                "eyeprotect" => PageColorMode::EyeProtect,
                _ => PageColorMode::White,
            }
        } else {
            PageColorMode::White
        };

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
            worker_state: WorkerState::Loading,
            last_rem_size: 16.0,
            window_scale_factor: 1.0,
            last_render_scale_factor: 0.0,
            last_zoom_level: if initial_state.zoom_level > 0.1 {
                initial_state.zoom_level
            } else {
                1.0
            },
            fit_to_width_mode: initial_state.fit_to_width,
            initial_state: initial_state.clone(),
            is_restoring: true,

            language,

            // 页面数据初始化（文档加载后会重置）
            page_images: Vec::new(),
            raw_page_images: Vec::new(),
            page_text_data: Vec::new(),
            page_link_data: Vec::new(),
            thumbnail_images: Vec::new(),
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
            thumbnail_list_state,
            is_right_sidebar_open: initial_state.is_right_sidebar_open,
            active_right_sidebar_tab: RightSidebarTab::Translation,
            translation_result: None,
            engine_select: None,
            translation_original_expanded: initial_state.translation_original_expanded,
            translation_font_size: initial_state.translation_font_size,
            auto_translate: initial_state.auto_translate,

            preferred_left_sidebar_width: if initial_state.left_sidebar_width > 0.0 {
                initial_state.left_sidebar_width
            } else {
                DEFAULT_LEFT_SIDEBAR_WIDTH
            },
            preferred_right_sidebar_width: if initial_state.right_sidebar_width > 0.0 {
                initial_state.right_sidebar_width
            } else {
                DEFAULT_RIGHT_SIDEBAR_WIDTH
            },
            left_sidebar_width: px(if initial_state.left_sidebar_width > 0.0 {
                initial_state.left_sidebar_width
            } else {
                DEFAULT_LEFT_SIDEBAR_WIDTH
            }),
            right_sidebar_width: px(if initial_state.right_sidebar_width > 0.0 {
                initial_state.right_sidebar_width
            } else {
                DEFAULT_RIGHT_SIDEBAR_WIDTH
            }),
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
            annotation_drag: None,
            page_color_mode: initial_page_color_mode,
            tab_bar_offset_rems: 0.0,
        }
    }

    pub fn set_tab_bar_offset_rems(&mut self, rems: f32) {
        self.tab_bar_offset_rems = rems;
    }

    pub(crate) fn get_page_color_rgb(&self) -> Option<(u8, u8, u8)> {
        self.page_color_mode.to_rgb_tuple()
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

        // 清空现有页面图像并重新发送渲染请求，使新色彩滤镜生效
        for img in self.page_images.iter_mut() {
            *img = None;
        }
        for img in self.raw_page_images.iter_mut() {
            *img = None;
        }

        if self.worker_state == WorkerState::Running {
            for page in 0..self.total_pages as u16 {
                let page_scale = self.render_zoom * self.window_scale_factor * 1.2;
                self.pdf_service.send_render(page, page_scale, 0);
                let display_w = PAGE_BASE_WIDTH_REMS * self.zoom_level * self.last_rem_size;
                let (pdf_w, pdf_h) = self.page_sizes.get(page as usize).copied().unwrap_or((612.0, 792.0));
                let display_h = display_w * (pdf_h / pdf_w);
                self.pdf_service.send_text(page, display_w, display_h, 0);
                self.pdf_service.send_links(page, display_w, display_h, 0);
            }
        }

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

                                            // 发送所有页面的渲染请求（使用 window_scale_factor 适配 HiDPI/Retina）
                                            let page_scale = this.render_zoom * this.window_scale_factor * 1.2;
                                            let display_w = PAGE_BASE_WIDTH_REMS * this.zoom_level * this.last_rem_size;
                                            for page in 0..page_count as u16 {
                                                this.pdf_service.send_render(page, page_scale, 0);
                                                let (pdf_w, pdf_h) = this.page_sizes.get(page as usize).copied().unwrap_or((612.0, 792.0));
                                                let display_h = display_w * (pdf_h / pdf_w);
                                                this.pdf_service.send_text(page, display_w, display_h, 0);
                                                this.pdf_service.send_links(page, display_w, display_h, 0);
                                                this.pdf_service.send_thumbnail_render(page, 250.0, 0);
                                            }

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
                                            if let Some(slot) = this.page_link_data.get_mut(page as usize) {
                                                *slot = Some(Arc::new(data));
                                            }
                                            cx.notify();
                                        }
                                        PdfResponse::TextExtracted {
                                            page,
                                            generation: _,
                                            data,
                                        } => {
                                            this.on_text_extracted(page, data, cx);
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
            let tab_bar_h = self.tab_bar_offset_rems * f32::from(window.rem_size());
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

        // 记录当前窗口的 scale_factor（HiDPI/Retina 显示）和 rem_size
        let scale_factor = window.scale_factor();
        let rem_size = window.rem_size();
        self.window_scale_factor = scale_factor;
        self.last_rem_size = f32::from(rem_size);

        // 如果 scale_factor 首次确定或发生变化，重新发送渲染请求以匹配 HiDPI
        if self.worker_state == WorkerState::Running
            && (self.last_render_scale_factor - scale_factor).abs() > 0.01
        {
            self.last_render_scale_factor = scale_factor;
            for page in 0..self.total_pages as u16 {
                let page_scale = self.render_zoom * scale_factor * 1.2;
                self.pdf_service.send_render(page, page_scale, 0);
                let display_w = PAGE_BASE_WIDTH_REMS * self.zoom_level * f32::from(rem_size);
                let (pdf_w, pdf_h) = self.page_sizes.get(page as usize).copied().unwrap_or((612.0, 792.0));
                let display_h = display_w * (pdf_h / pdf_w);
                self.pdf_service.send_text(page, display_w, display_h, 0);
                self.pdf_service.send_links(page, display_w, display_h, 0);
            }
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
