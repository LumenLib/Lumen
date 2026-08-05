//! PDF 阅读器持久化（注解）
//!
//! 仅做 DB 编排，收 `&Database` / `Arc<Database>`，不感知 UI / 同步
//! （架构红线）。`notify_data_changed` 等跨域副作用由调用方
//! （`AppPdfDelegate`）负责。

use database::Database;
use models::Annotation;

pub struct PdfPersistence;

impl PdfPersistence {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for PdfPersistence {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfPersistence {
    // ── 注解 ────────────────────────────────────────────

    pub fn load_annotations(&self, db: &Database, id: &str) -> Vec<Annotation> {
        db.load_annotations(id).unwrap_or_default()
    }

    pub fn save_annotation(&self, db: &Database, annotation: &Annotation) {
        let _ = db.save_annotation(annotation);
    }

    pub fn delete_annotation(&self, db: &Database, id: &str) {
        let _ = db.delete_annotation(id);
    }
}
