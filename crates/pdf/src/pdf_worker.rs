use image::{ImageBuffer, RgbaImage};
use log::{debug, error, info};
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
        doc_id: u32,
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
            // 优先级 1：Shutdown 信号
            if let Some(idx) = q.iter().position(|r| matches!(r, PdfRequest::Shutdown)) {
                return q.remove(idx).unwrap();
            }
            // 优先级 2：文档打开与关闭
            if let Some(idx) = q.iter().position(|r| matches!(r, PdfRequest::OpenDocument { .. } | PdfRequest::CloseDocument { .. })) {
                return q.remove(idx).unwrap();
            }
            // 优先级 3：主页面渲染 (RenderPage)
            if let Some(idx) = q.iter().position(|r| matches!(r, PdfRequest::RenderPage { .. })) {
                return q.remove(idx).unwrap();
            }
            // 优先级 4：文本与链接数据准备
            if let Some(idx) = q.iter().position(|r| matches!(r, PdfRequest::ExtractText { .. } | PdfRequest::ExtractLinks { .. })) {
                return q.remove(idx).unwrap();
            }
            // 优先级 5：如果没有高优先级任务，正常处理剩下的任务（如缩略图 RenderThumbnail）
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

fn extract_outlines(outlines: &[mupdf::Outline]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    for outline in outlines {
        let title = outline.title.clone();
        let page_index = outline
            .dest
            .map(|dest| dest.loc.page_number as u16)
            .unwrap_or(0);
        let children = extract_outlines(&outline.down);
        items.push(OutlineItem {
            title,
            page_index,
            children,
        });
    }
    items
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

        let mut documents: HashMap<u32, (mupdf::Document, SyncSender<PdfResponse>)> =
            HashMap::new();
        loop {
            let request = queue.pop();
            match request {
                PdfRequest::Shutdown => {
                    info!("PDF Global Worker: 收到全局关闭信号");
                    break;
                }
                PdfRequest::OpenDocument { doc_id, path, tx } => {
                    info!(
                        "PDF Global Worker: 正在加载文档 {}, ID: {}",
                        path.display(),
                        doc_id
                    );
                    let path_str = match path.to_str() {
                        Some(p) => p,
                        None => {
                            let _ =
                                tx.send(PdfResponse::FatalError("Path is not valid UTF-8".into()));
                            continue;
                        }
                    };
                    match mupdf::Document::open(path_str) {
                        Ok(document) => {
                            let page_count = match document.page_count() {
                                Ok(c) => c as usize,
                                Err(e) => {
                                    let _ = tx.send(PdfResponse::FatalError(format!(
                                        "Failed to get page count: {e:?}"
                                    )));
                                    continue;
                                }
                            };
                            let mut page_sizes = Vec::with_capacity(page_count);
                            for i in 0..page_count {
                                if let Ok(page) = document.load_page(i as i32) {
                                    if let Ok(bounds) = page.bounds() {
                                        page_sizes
                                            .push((bounds.x1 - bounds.x0, bounds.y1 - bounds.y0));
                                    } else {
                                        page_sizes.push((612.0, 792.0));
                                    }
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
                            if let Ok(outlines) = document.outlines() {
                                let mapped_outlines = extract_outlines(&outlines);
                                if !mapped_outlines.is_empty() {
                                    let _ = tx.send(PdfResponse::OutlineExtracted {
                                        doc_id,
                                        outlines: mapped_outlines,
                                    });
                                }
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
                        let pdf_page = match document.load_page(page as i32) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let matrix = mupdf::Matrix::new_scale(scale, scale);
                        let pixmap = match pdf_page.to_pixmap(
                            &matrix,
                            &mupdf::Colorspace::device_bgr(),
                            true,
                            true,
                        ) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx
                                    .send(PdfResponse::FatalError(format!("Render error: {e:?}")));
                                continue;
                            }
                        };

                        let width = pixmap.width();
                        let height = pixmap.height();
                        let rgba_bytes = pixmap.samples().to_vec();

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
                        let pdf_page = match document.load_page(page as i32) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let bounds = match pdf_page.bounds() {
                            Ok(b) => b,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page bounds: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let base_w = bounds.x1 - bounds.x0;
                        let base_h = bounds.y1 - bounds.y0;
                        let scale = max_size / base_w.max(base_h);

                        let matrix = mupdf::Matrix::new_scale(scale, scale);
                        let pixmap = match pdf_page.to_pixmap(
                            &matrix,
                            &mupdf::Colorspace::device_bgr(),
                            true,
                            true,
                        ) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx
                                    .send(PdfResponse::FatalError(format!("Render error: {e:?}")));
                                continue;
                            }
                        };

                        let width = pixmap.width();
                        let height = pixmap.height();
                        let rgba_bytes = pixmap.samples().to_vec();

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
                        let pdf_page = match document.load_page(page as i32) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let bounds = match pdf_page.bounds() {
                            Ok(b) => b,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page bounds: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let page_width = bounds.x1 - bounds.x0;
                        let page_height = bounds.y1 - bounds.y0;
                        let scale = (display_w / page_width + display_h / page_height) / 2.0;

                        let mut links_data = Vec::new();
                        if let Ok(links_iter) = pdf_page.links() {
                            for link in links_iter {
                                let rect = link.bounds;
                                let left = rect.x0 * scale;
                                let top = rect.y0 * scale;
                                let right = rect.x1 * scale;
                                let bottom = rect.y1 * scale;

                                links_data.push(LinkInfo {
                                    left,
                                    top,
                                    right,
                                    bottom,
                                    url: link.uri,
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
                        let pdf_page = match document.load_page(page as i32) {
                            Ok(p) => p,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let bounds = match pdf_page.bounds() {
                            Ok(b) => b,
                            Err(e) => {
                                let _ = tx.send(PdfResponse::FatalError(format!(
                                    "Failed to get page bounds: {e:?}"
                                )));
                                continue;
                            }
                        };

                        let page_width = bounds.x1 - bounds.x0;
                        let page_height = bounds.y1 - bounds.y0;
                        let scale = (display_w / page_width + display_h / page_height) / 2.0;

                        let text_page = match pdf_page.to_text_page(mupdf::TextPageFlags::empty()) {
                            Ok(t) => t,
                            Err(e) => {
                                error!("PDF Worker: 文本提取失败 - 页面 {}: {e:?}", page);
                                continue;
                            }
                        };

                        let mut all_chars: Vec<TextChar> = Vec::new();
                        let mut i = 0;

                        for block in text_page.blocks() {
                            if block.r#type() == mupdf::text_page::TextBlockType::Text {
                                for line in block.lines() {
                                    for ch in line.chars() {
                                        let quad = ch.quad();
                                        let unicode_char = ch.char().unwrap_or(' ');
                                        let origin = ch.origin();
                                        let font_size = ch.size();

                                        let ascender = 0.75;
                                        let descender = -0.2;

                                        let pdf_top = origin.y - font_size * ascender;
                                        let pdf_bottom = origin.y - font_size * descender;

                                        let left = [quad.ul.x, quad.ur.x, quad.ll.x, quad.lr.x]
                                            .into_iter()
                                            .fold(f32::MAX, f32::min)
                                            * scale;
                                        let right = [quad.ul.x, quad.ur.x, quad.ll.x, quad.lr.x]
                                            .into_iter()
                                            .fold(f32::MIN, f32::max)
                                            * scale;

                                        let x = left;
                                        let top = pdf_top * scale;
                                        let width = right - left;
                                        let height = (pdf_bottom - pdf_top) * scale;
                                        let baseline = ch.origin().y * scale;

                                        all_chars.push(TextChar {
                                            id: i,
                                            char: unicode_char,
                                            x,
                                            y: top,
                                            width,
                                            height,
                                            font_size: font_size * scale,
                                            baseline,
                                        });
                                        i += 1;
                                    }
                                }
                            }
                        }

                        debug!(
                            "PDF Worker: 文本提取成功 - 页面 {}, 共 {} 个字符",
                            page,
                            all_chars.len()
                        );
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
