//! 搜索相关类型
//!
//! `AdvancedSearchQuery` / `SearchField` 已下沉 `models`（纯数据），
//! `SearchEngine` 已下沉 `crates/services::query`（匹配原语）。此处仅保留 UI 专属的
//! `SearchMatch`，并对已下沉类型做重导出，保持旧路径 `ui::views::toolbar::*`
//! 不变。

/// 搜索匹配结果
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// 匹配度分数 (可选，用于未来排序)
    pub score: f32,
    /// 匹配的字段类别 (Title, Author, Journal)
    pub field: SearchField,
}

// 纯数据类型下沉到 models，此处重导出以保持旧路径可用
pub use models::{AdvancedSearchQuery, SearchField};

// 搜索逻辑下沉到 services crate，此处重导出以保持旧路径可用
pub use services::query::SearchEngine;
