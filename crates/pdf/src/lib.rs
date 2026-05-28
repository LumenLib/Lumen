pub use annotation::*;
use anyhow::Result;
use i18n::Language;
use log::{debug, info};
pub use models::{Annotation, AnnotationColor, AnnotationKind, TextRange};
pub use pdf_worker::*;
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, sync_channel},
    },
};
pub use view::PdfReaderView;

mod annotation;
mod pdf_worker;
mod view;

/// 单个字符的文本信息
#[derive(Debug, Clone)]
pub struct TextChar {
    pub id: usize,
    pub char: char,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub font_size: f32,
}

/// 页面的所有文本数据
#[derive(Debug, Clone)]
pub struct TextPageData {
    pub chars: Vec<TextChar>,
    /// 生成此数据时的 display_width_px，用于缓存键验证
    pub display_w: f32,
}

const Y_TOLERANCE_FACTOR: f32 = 0.3;

impl TextPageData {
    /// 将 [start, end] 内的字符合并成若干个视觉行块。
    /// 返回 Vec<(left, top, right, bottom)>，每个元素是一个连续块的边界。
    pub(crate) fn merge_char_blocks(&self, start: usize, end: usize) -> Vec<(f32, f32, f32, f32)> {
        let mut blocks = Vec::new();
        let mut current_block: Option<(f32, f32, f32, f32)> = None;

        for i in start..=end {
            if let Some(ch) = self.chars.get(i) {
                if let Some((bx, by, b_max_x, b_max_y)) = current_block {
                    let y_tolerance = ch.height * Y_TOLERANCE_FACTOR;
                    let char_top = ch.y - y_tolerance;
                    let char_bottom = ch.y + ch.height + y_tolerance;
                    let overlaps_vertically = char_top < b_max_y && char_bottom > by;

                    if overlaps_vertically {
                        current_block = Some((
                            bx.min(ch.x),
                            by.min(ch.y),
                            b_max_x.max(ch.x + ch.width),
                            b_max_y.max(ch.y + ch.height),
                        ));
                    } else if ch.width <= 0.0 {
                        current_block = Some((
                            bx.min(ch.x),
                            by.min(ch.y),
                            b_max_x.max(ch.x),
                            b_max_y.max(ch.y + ch.height),
                        ));
                    } else {
                        blocks.push((bx, by, b_max_x, b_max_y));
                        current_block = Some((ch.x, ch.y, ch.x + ch.width, ch.y + ch.height));
                    }
                } else {
                    current_block = Some((ch.x, ch.y, ch.x + ch.width, ch.y + ch.height));
                }
            }
        }

        if let Some((bx, by, b_max_x, b_max_y)) = current_block {
            blocks.push((bx, by, b_max_x, b_max_y));
        }

        blocks
    }
}

/// PDF 大纲（书签）节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineItem {
    pub title: String,
    pub page_index: u16,            // 点击后跳转的目标页码
    pub children: Vec<OutlineItem>, // 嵌套的子大纲列表
}

/// PDF 链接信息
#[derive(Debug, Clone)]
pub struct LinkInfo {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub url: String,
}

/// 页面的所有链接数据
#[derive(Debug, Clone)]
pub struct LinkPageData {
    pub links: Vec<LinkInfo>,
    /// 生成此数据时的 display 尺寸，用于缓存键验证
    pub display_w: f32,
    pub display_h: f32,
}

/// 量化缩放级别 - 减少缓存变体
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoomLevel {
    VerySmall = 0, // 0.25x - 0.5x
    Small = 1,     // 0.5x - 0.75x
    Normal = 2,    // 0.75x - 1.25x
    Large = 3,     // 1.25x - 1.75x
    VeryLarge = 4, // 1.75x - 2.5x
    Huge = 5,      // 2.5x+
}

impl From<f32> for ZoomLevel {
    fn from(zoom: f32) -> Self {
        match zoom {
            z if z <= 0.5 => ZoomLevel::VerySmall,
            z if z <= 0.75 => ZoomLevel::Small,
            z if z <= 1.25 => ZoomLevel::Normal,
            z if z <= 1.75 => ZoomLevel::Large,
            z if z <= 2.5 => ZoomLevel::VeryLarge,
            _ => ZoomLevel::Huge,
        }
    }
}

/// 缓存键设计
#[derive(Hash, PartialEq, Eq, Clone, Debug)]
pub struct CacheKey {
    pub page_index: u16,
    pub zoom_level: ZoomLevel,
}

/// PDF 的初始状态
#[derive(Debug, Clone)]
pub struct PdfInitialState {
    pub page_index: u16,
    pub zoom_level: f32,
    pub offset_y: f32,
    pub fit_to_width: bool,
    pub auto_translate: bool,
    pub is_left_sidebar_open: bool,
    pub is_right_sidebar_open: bool,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
    pub translation_font_size: f32,
    pub translation_original_expanded: bool,
}

impl Default for PdfInitialState {
    fn default() -> Self {
        Self {
            page_index: 0,
            zoom_level: 1.0,
            offset_y: 0.0,
            fit_to_width: false,
            auto_translate: true,
            is_left_sidebar_open: false,
            is_right_sidebar_open: false,
            left_sidebar_width: 0.0,
            right_sidebar_width: 0.0,
            translation_font_size: 14.0,
            translation_original_expanded: true,
        }
    }
}

/// PDF 阅读器委托，用于处理跨模块交互
pub trait PdfReaderDelegate: Send + Sync + 'static {
    /// 获取初始状态（从持久化存储中读取）
    fn get_initial_state(&self, _id: String) -> PdfInitialState {
        PdfInitialState::default()
    }

    /// 当阅读状态（页码或缩放）改变时触发
    fn save_state(
        &self,
        _id: String,
        _page: u16,
        _zoom: f32,
        _offset_y: f32,
        _fit_to_width: bool,
        _is_left_sidebar_open: bool,
        _is_right_sidebar_open: bool,
        _left_sidebar_width: f32,
        _right_sidebar_width: f32,
        _auto_translate: bool,
    ) {
    }

    /// 翻译文本
    fn translate(&self, _text: String) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        Box::pin(async {
            Ok(i18n::t(
                i18n::I18nKey::TranslationNotImplemented,
                i18n::Language::ZhCn,
            )
            .to_string())
        })
    }

    /// 获取可用的翻译引擎列表
    fn get_translation_engines(&self) -> Vec<String> {
        vec!["google_free".to_string(), "bing_free".to_string()]
    }

    /// 获取当前翻译引擎 ID
    fn current_translation_engine_id(&self) -> String {
        "google_free".to_string()
    }

    /// 切换翻译引擎
    fn set_translation_engine(&self, _name: String) {}

    /// 加载文档的所有注释
    fn load_annotations(&self, _document_id: &str) -> Vec<Annotation> {
        vec![]
    }

    /// 保存单条注释
    fn save_annotation(&self, _annotation: &Annotation) {}

    /// 删除单条注释
    fn delete_annotation(&self, _id: &str) {}

    /// 点击 PDF 链接时的回调
    fn on_link_click(&self, _url: String) {}

    /// 获取当前语言
    fn current_language(&self) -> Language {
        Language::ZhCn
    }

    /// 设置翻译字体大小
    fn set_translation_font_size(&self, _size: f32) {}

    /// 获取当前翻译字体大小
    fn translation_font_size(&self) -> f32 {
        14.0
    }

    /// 设置原文框展开/收起（全局持久化）
    fn set_translation_original_expanded(&self, _expanded: bool) {}

    /// 获取文献笔记（Markdown 文本）
    fn get_notes(&self, _id: &str) -> Option<String> {
        None
    }

    /// 保存文献笔记
    fn save_notes(&self, _id: &str, _notes: &str) {}
}

pub struct PdfService {
    task_queue: Arc<PdfTaskQueue>,
    doc_id: Arc<Mutex<Option<u32>>>,
}

impl PdfService {
    pub fn new(path: PathBuf) -> Result<(Arc<Self>, Receiver<PdfResponse>)> {
        info!("PdfService: 正在请求加载文档: {:?}", path);

        let task_queue = get_global_pdf_queue();
        let (response_tx, response_rx) = sync_channel(100);

        // 发送 OpenDocument 请求
        task_queue.push(PdfRequest::OpenDocument {
            path,
            tx: response_tx,
        });

        let doc_id = Arc::new(Mutex::new(None));
        let service = Arc::new(Self { task_queue, doc_id });

        Ok((service, response_rx))
    }

    pub fn set_doc_id(&self, id: u32) {
        let mut lock = self.doc_id.lock().expect("Failed to lock doc_id");
        *lock = Some(id);
    }

    pub fn get_doc_id(&self) -> Option<u32> {
        let lock = self.doc_id.lock().expect("Failed to lock doc_id");
        *lock
    }

    pub fn send_render(&self, page: u16, scale: f32, generation: u64) {
        if let Some(doc_id) = self.get_doc_id() {
            debug!(
                "PdfService: 发送渲染请求 - 页面: {}, 缩放: {}, 代数: {}",
                page, scale, generation
            );
            self.task_queue.push(PdfRequest::RenderPage {
                doc_id,
                page,
                scale,
                generation,
            });
        }
    }

    pub fn send_thumbnail_render(&self, page: u16, max_size: f32, generation: u64) {
        if let Some(doc_id) = self.get_doc_id() {
            debug!(
                "PdfService: 发送缩略图请求 - 页面: {}, 最大尺寸: {}, 代数: {}",
                page, max_size, generation
            );
            self.task_queue.push(PdfRequest::RenderThumbnail {
                doc_id,
                page,
                max_size,
                generation,
            });
        }
    }

    pub fn send_links(&self, page: u16, display_w: f32, display_h: f32, generation: u64) {
        if let Some(doc_id) = self.get_doc_id() {
            debug!(
                "PdfService: 发送链接请求 - 页面: {}, 代数: {}",
                page, generation
            );
            self.task_queue.push(PdfRequest::ExtractLinks {
                doc_id,
                page,
                display_w,
                display_h,
                generation,
            });
        }
    }

    pub fn send_text(&self, page: u16, display_w: f32, display_h: f32, generation: u64) {
        if let Some(doc_id) = self.get_doc_id() {
            debug!(
                "PdfService: 发送文本请求 - 页面: {}, 代数: {}",
                page, generation
            );
            self.task_queue.push(PdfRequest::ExtractText {
                doc_id,
                page,
                display_w,
                display_h,
                generation,
            });
        }
    }
}

impl Drop for PdfService {
    fn drop(&mut self) {
        info!("PdfService: 清理文档");
        if let Some(doc_id) = self.get_doc_id() {
            info!("PdfService: 发送 CloseDocument 请求, doc_id={}", doc_id);
            self.task_queue.push(PdfRequest::CloseDocument { doc_id });
        }
    }
}
