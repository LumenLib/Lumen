//! 只读查询域（query）
//!
//! 纯派生查询（`sort_literatures` / `search_literatures` / `get_folder_literatures` 等），
//! 无 DB 写入、无 GPUI 依赖。
//!
//! 其中 `SearchEngine`（见 `engine.rs`）为底层匹配原语，依赖
//! `parser::normalize`，被本域的 `search_literatures` 等上层编排调用。

pub mod data;
mod engine;
pub use data::{
    AppViewMode, SortField, SortOrder, get_feed_items, get_folder_literatures, search_literatures,
    sort_literatures,
};
pub use engine::SearchEngine;
