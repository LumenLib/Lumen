use anyhow::Result;
use image::{ImageBuffer, RgbaImage};
use log::{debug, error, info};
use pdfium_render::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;

use crate::{LinkInfo, LinkPageData, OutlineItem, TextChar, TextPageData};

// ─── Worker Messages ─────────────────────────────────────────

#[derive(Debug)]
pub enum PdfRequest {
    /// 请求加载文档
    OpenDocument {
        path: PathBuf,
        tx: SyncSender<PdfResponse>,
    },
    /// 请求渲染特定页面
    RenderPage {
        doc_id: u32,
        page: u16,
        scale: f32,
        generation: u64,
    },
    /// 请求渲染缩略图
    RenderThumbnail {
        doc_id: u32,
        page: u16,
        max_size: f32,
        generation: u64,
    },
    /// 请求提取特定页面的链接
    ExtractLinks {
        doc_id: u32,
        page: u16,
        display_w: f32,
        display_h: f32,
        generation: u64,
    },
    /// 请求提取特定页面的文本数据
    ExtractText {
        doc_id: u32,
        page: u16,
        display_w: f32,
        display_h: f32,
        generation: u64,
    },
    /// 关闭特定文档
    CloseDocument { doc_id: u32 },
    /// 关闭工作线程
    Shutdown,
}

fn is_same_task(a: &PdfRequest, b: &PdfRequest) -> bool {
    match (a, b) {
        (
            PdfRequest::RenderPage {
                doc_id: d1,
                page: p1,
                ..
            },
            PdfRequest::RenderPage {
                doc_id: d2,
                page: p2,
                ..
            },
        ) => d1 == d2 && p1 == p2,
        (
            PdfRequest::ExtractLinks {
                doc_id: d1,
                page: p1,
                ..
            },
            PdfRequest::ExtractLinks {
                doc_id: d2,
                page: p2,
                ..
            },
        ) => d1 == d2 && p1 == p2,
        (
            PdfRequest::ExtractText {
                doc_id: d1,
                page: p1,
                ..
            },
            PdfRequest::ExtractText {
                doc_id: d2,
                page: p2,
                ..
            },
        ) => d1 == d2 && p1 == p2,
        (
            PdfRequest::RenderThumbnail {
                doc_id: d1,
                page: p1,
                ..
            },
            PdfRequest::RenderThumbnail {
                doc_id: d2,
                page: p2,
                ..
            },
        ) => d1 == d2 && p1 == p2,
        _ => false,
    }
}

pub struct PdfTaskQueue {
    queue: Mutex<VecDeque<PdfRequest>>,
    condvar: Condvar,
    max_capacity: usize,
}

impl PdfTaskQueue {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            condvar: Condvar::new(),
            max_capacity,
        }
    }

    pub fn push(&self, req: PdfRequest) {
        let mut q = self.queue.lock().unwrap();

        // 去重：移除同类型的旧请求
        q.retain(|existing| !is_same_task(existing, &req));

        // 加入新请求到队尾
        q.push_back(req);

        // 截断：如果超出容量，丢弃队首（最老）的任务
        if q.len() > self.max_capacity {
            // 避免丢弃关键的任务
            let mut remove_idx = None;
            for (i, r) in q.iter().enumerate() {
                match r {
                    PdfRequest::OpenDocument { .. } | PdfRequest::Shutdown => continue,
                    _ => {
                        remove_idx = Some(i);
                        break;
                    }
                }
            }
            if let Some(idx) = remove_idx {
                q.remove(idx);
            }
        }
        self.condvar.notify_one();
    }

    pub fn pop(&self) -> PdfRequest {
        let mut q = self.queue.lock().unwrap();
        loop {
            if let Some(req) = q.pop_front() {
                return req;
            }
            q = self.condvar.wait(q).unwrap();
        }
    }
}

#[derive(Debug)]
pub enum PdfResponse {
    /// 文档加载完成
    DocumentLoaded {
        doc_id: u32,
        page_count: usize,
        page_sizes: Vec<(f32, f32)>,
    },
    /// 页面渲染完成
    PageRendered {
        page: u16,
        generation: u64,
        image: RgbaImage,
    },
    /// 缩略图渲染完成
    ThumbnailRendered {
        page: u16,
        generation: u64,
        image: RgbaImage,
    },
    /// 文本提取完成
    TextExtracted {
        page: u16,
        generation: u64,
        data: TextPageData,
    },
    /// 链接提取完成
    LinksExtracted {
        page: u16,
        generation: u64,
        data: LinkPageData,
    },
    /// 提取出文档的大纲结构
    OutlineExtracted {
        doc_id: u32,
        outlines: Vec<OutlineItem>,
    },
    /// 致命错误（如文档无法打开）
    FatalError(String),
}

// ─── Worker Core ─────────────────────────────────────────────

fn extract_bookmarks<'a>(iter: impl Iterator<Item = PdfBookmark<'a>>) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    for bookmark in iter {
        let title = bookmark.title().unwrap_or_else(|| "未命名书签".to_string());

        let page_index = bookmark
            .destination()
            .and_then(|dest| dest.page_index().ok())
            .unwrap_or(0);

        let children = extract_bookmarks(bookmark.iter_direct_children());

        items.push(OutlineItem {
            title,
            page_index,
            children,
        });
    }
    items
}

pub(crate) fn create_pdfium() -> Result<Pdfium> {
    debug!("PDF Worker: 正在绑定本地库 (bin/)...");

    // 强制将加载路径定向为可执行程序同级的 bin 目录
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let bin_dir = parent.join("bin");
            let _ = std::fs::create_dir_all(&bin_dir);

            let lib_filename = if cfg!(target_os = "windows") {
                "pdfium.dll"
            } else if cfg!(target_os = "macos") {
                "libpdfium.dylib"
            } else {
                "libpdfium.so"
            };
            let local_lib_path = bin_dir.join(lib_filename);

            debug!("PDF Worker: 强制加载本地库路径 {:?}", local_lib_path);
            unsafe {
                std::env::set_var("PDFIUM_LIB_PATH", &local_lib_path);
            }
        }
    }

    let pdfium = pdfium_auto::bind_pdfium_silent()
        .map_err(|e| anyhow::anyhow!("PDFium 绑定失败: {e:?}"))?;
    debug!("PDF Worker: PDFium 绑定成功");
    Ok(pdfium)
}

pub fn get_global_pdf_queue() -> Arc<PdfTaskQueue> {
    static GLOBAL_QUEUE: OnceLock<Arc<PdfTaskQueue>> = OnceLock::new();
    GLOBAL_QUEUE
        .get_or_init(|| {
            let queue = Arc::new(PdfTaskQueue::new(200));
            start_global_worker(Arc::clone(&queue));
            queue
        })
        .clone()
}

fn start_global_worker(queue: Arc<PdfTaskQueue>) {
    thread::spawn(move || {
        info!("PDF Global Worker: 线程已启动");
        let pdfium = match create_pdfium() {
            Ok(p) => p,
            Err(e) => {
                error!("PDF Global Worker: PDFium 初始化失败: {e}");
                return;
            }
        };

        let mut documents: HashMap<u32, (PdfDocument, SyncSender<PdfResponse>)> = HashMap::new();
        let mut next_doc_id = 1;

        loop {
            let request = queue.pop();
            match request {
                PdfRequest::Shutdown => {
                    info!("PDF Global Worker: 收到全局关闭信号");
                    break;
                }
                PdfRequest::OpenDocument { path, tx } => {
                    let doc_id = next_doc_id;
                    next_doc_id += 1;

                    info!(
                        "PDF Global Worker: 正在加载文档 {}, ID: {}",
                        path.display(),
                        doc_id
                    );
                    match pdfium.load_pdf_from_file(&path, None) {
                        Ok(document) => {
                            let page_count = document.pages().len() as usize;
                            let mut page_sizes = Vec::with_capacity(page_count);
                            for i in 0..page_count {
                                if let Ok(page) = document.pages().get(PdfPageIndex::from(i as u16))
                                {
                                    page_sizes.push((page.width().value, page.height().value));
                                } else {
                                    page_sizes.push((612.0, 792.0));
                                }
                            }
                            let _ = tx.send(PdfResponse::DocumentLoaded {
                                doc_id,
                                page_count,
                                page_sizes,
                            });

                            // 提取并发送大纲数据
                            let bookmarks = document.bookmarks();
                            let outlines = extract_bookmarks(bookmarks.iter());
                            if !outlines.is_empty() {
                                let _ = tx.send(PdfResponse::OutlineExtracted { doc_id, outlines });
                            }

                            documents.insert(doc_id, (document, tx));
                        }
                        Err(e) => {
                            error!("PDF Global Worker: 文档加载失败: {e:?}");
                            let _ = tx.send(PdfResponse::FatalError(format!(
                                "Failed to load PDF: {e:?}"
                            )));
                        }
                    }
                }
                PdfRequest::CloseDocument { doc_id } => {
                    if documents.remove(&doc_id).is_some() {
                        info!("PDF Global Worker: 文档 {} 已关闭", doc_id);
                    }
                }
                PdfRequest::RenderPage {
                    doc_id,
                    page,
                    scale,
                    generation,
                } => {
                    if let Some((document, tx)) = documents.get(&doc_id) {
                        let start_time = std::time::Instant::now();
                        debug!("PDF Worker: 开始渲染页面 {}, 代数 {}", page, generation);
                        let pdf_page = match document.pages().get(PdfPageIndex::from(page)) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let render_config = PdfRenderConfig::new()
                            .scale_page_by_factor(scale)
                            .set_reverse_byte_order(false);

                        let bitmap = match pdf_page.render_with_config(&render_config) {
                            Ok(b) => b,
                            Err(e) => {
                                let _ = tx
                                    .send(PdfResponse::FatalError(format!("Render error: {e:?}")));
                                continue;
                            }
                        };

                        let width = bitmap.width() as u32;
                        let height = bitmap.height() as u32;
                        let rgba_bytes = bitmap.as_raw_bytes().to_vec();

                        if let Some(img) = ImageBuffer::from_raw(width, height, rgba_bytes) {
                            info!(
                                "PDF Worker: 渲染成功 - 页面 {}, 分辨率 {}x{}, 耗时 {:?}",
                                page,
                                width,
                                height,
                                start_time.elapsed()
                            );

                            let _ = tx.send(PdfResponse::PageRendered {
                                page,
                                generation,
                                image: img,
                            });
                        } else {
                            error!("PDF Worker: ImageBuffer 创建失败 - 页面 {}", page);
                            let _ = tx.send(PdfResponse::FatalError(
                                "ImageBuffer creation failed".into(),
                            ));
                        }
                    }
                }
                PdfRequest::RenderThumbnail {
                    doc_id,
                    page,
                    max_size,
                    generation,
                } => {
                    if let Some((document, tx)) = documents.get(&doc_id) {
                        let start_time = std::time::Instant::now();
                        debug!("PDF Worker: 开始渲染缩略图 {}, 代数 {}", page, generation);
                        let pdf_page = match document.pages().get(PdfPageIndex::from(page)) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let (base_w, base_h) = (pdf_page.width().value, pdf_page.height().value);
                        let scale = max_size / base_w.max(base_h);

                        let render_config = PdfRenderConfig::new()
                            .scale_page_by_factor(scale)
                            .set_reverse_byte_order(false);

                        let bitmap = match pdf_page.render_with_config(&render_config) {
                            Ok(b) => b,
                            Err(e) => {
                                let _ = tx
                                    .send(PdfResponse::FatalError(format!("Render error: {e:?}")));
                                continue;
                            }
                        };

                        let width = bitmap.width() as u32;
                        let height = bitmap.height() as u32;
                        let rgba_bytes = bitmap.as_raw_bytes().to_vec();

                        if let Some(img) = ImageBuffer::from_raw(width, height, rgba_bytes) {
                            info!(
                                "PDF Worker: 缩略图渲染成功 - 页面 {}, 分辨率 {}x{}, 耗时 {:?}",
                                page,
                                width,
                                height,
                                start_time.elapsed()
                            );
                            let _ = tx.send(PdfResponse::ThumbnailRendered {
                                page,
                                generation,
                                image: img,
                            });
                        }
                    }
                }
                PdfRequest::ExtractLinks {
                    doc_id,
                    page,
                    display_w,
                    display_h,
                    generation,
                } => {
                    if let Some((document, tx)) = documents.get(&doc_id) {
                        debug!("PDF Worker: 开始提取页面 {} 的链接", page);
                        let pdf_page = match document.pages().get(PdfPageIndex::from(page)) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let page_width = pdf_page.width().value;
                        let page_height = pdf_page.height().value;
                        let scale = (display_w / page_width + display_h / page_height) / 2.0;

                        let page_links = pdf_page.links();
                        let mut links_data = Vec::new();

                        for link in page_links.iter() {
                            if let Ok(rect) = link.rect()
                                && let Some(action) = link.action()
                                && let Some(uri_action) = action.as_uri_action()
                                && let Ok(uri) = uri_action.uri()
                            {
                                let url = uri.to_string();
                                let left = rect.left().value * scale;
                                let top = display_h - (rect.top().value * scale);
                                let right = rect.right().value * scale;
                                let bottom = display_h - (rect.bottom().value * scale);

                                links_data.push(LinkInfo {
                                    left,
                                    top,
                                    right,
                                    bottom,
                                    url,
                                });
                            }
                        }

                        debug!(
                            "PDF Worker: 链接提取成功 - 页面 {}, 共 {} 个链接",
                            page,
                            links_data.len()
                        );
                        let _ = tx.send(PdfResponse::LinksExtracted {
                            page,
                            generation,
                            data: LinkPageData {
                                links: links_data,
                                display_w,
                                display_h,
                            },
                        });
                    }
                }

                PdfRequest::ExtractText {
                    doc_id,
                    page,
                    display_w,
                    display_h,
                    generation,
                } => {
                    if let Some((document, tx)) = documents.get(&doc_id) {
                        debug!("PDF Worker: 开始提取页面 {} 的文本", page);
                        let pdf_page = match document.pages().get(PdfPageIndex::from(page)) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let page_width = pdf_page.width().value;
                        let page_height = pdf_page.height().value;
                        let scale = (display_w / page_width + display_h / page_height) / 2.0;

                        let page_text = match pdf_page.text() {
                            Ok(t) => t,
                            Err(e) => {
                                error!("PDF Worker: 文本提取失败 - 页面 {}: {e:?}", page);
                                continue;
                            }
                        };

                        let mut all_chars: Vec<TextChar> = Vec::new();

                        let chars = page_text.chars();

                        for i in 0..chars.len() {
                            if let Ok(char) = chars.get(i)
                                && let Ok(bounds) = char.tight_bounds()
                            {
                                let font_size = char.unscaled_font_size().value;
                                let unicode_char = char.unicode_char().unwrap_or(' ');

                                let pdf_x = bounds.left().value;
                                let pdf_y_top = bounds.bottom().value + bounds.height().value;

                                all_chars.push(TextChar {
                                    id: i,
                                    char: unicode_char,
                                    x: pdf_x * scale,
                                    y: display_h - (pdf_y_top * scale),
                                    width: bounds.width().value * scale,
                                    height: bounds.height().value * scale,
                                    font_size: font_size * scale,
                                });
                            }
                        }

                        debug!("PDF Worker: 文本提取成功 - 页面 {}", page);
                        let _ = tx.send(PdfResponse::TextExtracted {
                            page,
                            generation,
                            data: TextPageData {
                                chars: all_chars,
                                display_w,
                            },
                        });
                    }
                }
            }
        }
        info!("PDF Global Worker: 线程已退出");
    });
}
