//! 应用状态数据（纯查询层）
//!
//! 包含排序、筛选、搜索等只读派生查询，无 DB 写入、无 GPUI 依赖。
//! 已从 lumen `src/services/data.rs` 迁入 `services::query::data`（B-query）。

use crate::query::SearchEngine;
use log::debug;
use models::AdvancedSearchQuery;
use models::{FeedItem, Folder, Literature, Tag};
use parser::normalize::author_full_name;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// 应用视图模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppViewMode {
    Library,
    Subscription,
}

/// 排序字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortField {
    #[default]
    Title,
    Author,
    Year,
    Journal,
}

/// 排序顺序
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

// =============================================================================
// 自由函数：供 UI 层调用（纯内存派生，无 IO）
// =============================================================================

/// 对文献列表进行排序
#[must_use]
pub fn sort_literatures(
    mut literatures: Vec<&Arc<Literature>>,
    sort_field: SortField,
    sort_order: SortOrder,
) -> Vec<&Arc<Literature>> {
    debug!(
        "数据层: 排序 input={}, field={:?}, order={:?}",
        literatures.len(),
        sort_field,
        sort_order
    );
    literatures.sort_by(|a, b| {
        let cmp = match sort_field {
            SortField::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            SortField::Author => {
                let a_author = a.authors.first().map(author_full_name).unwrap_or_default();
                let b_author = b.authors.first().map(author_full_name).unwrap_or_default();
                a_author.to_lowercase().cmp(&b_author.to_lowercase())
            }
            SortField::Year => b.year.unwrap_or(0).cmp(&a.year.unwrap_or(0)),
            SortField::Journal => {
                let a_journal = a
                    .publication
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                let b_journal = b
                    .publication
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                a_journal.to_lowercase().cmp(&b_journal.to_lowercase())
            }
        };

        let primary = match sort_order {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        };

        if primary == std::cmp::Ordering::Equal {
            a.title.to_lowercase().cmp(&b.title.to_lowercase())
        } else {
            primary
        }
    });

    literatures
}

/// 获取当前筛选条件下的文献列表
#[must_use]
pub fn get_folder_literatures<'a>(
    literatures: &'a [Arc<Literature>],
    tags: &[(Arc<Tag>, usize)],
    selected_folder_id: &Option<String>,
    selected_tag_id: &Option<String>,
    sort_field: SortField,
    sort_order: SortOrder,
) -> Vec<&'a Arc<Literature>> {
    let mut results = if let Some(tag_id) = selected_tag_id.as_ref() {
        if let Some((tag, _)) = tags.iter().find(|(t, _)| t.id == *tag_id) {
            let v: Vec<&Arc<Literature>> = literatures
                .iter()
                .filter(|lit| lit.tags.contains(&tag.name))
                .collect();
            debug!(
                "数据层: 筛选标签 '{}'(id={}) => {} 条",
                tag.name,
                tag_id,
                v.len()
            );
            v
        } else {
            debug!("数据层: 标签 id={tag_id} 未找到，返回空");
            Vec::new()
        }
    } else if let Some(folder_id) = selected_folder_id.as_ref() {
        let v: Vec<&Arc<Literature>> = if folder_id == "all" {
            literatures
                .iter()
                .filter(|lit| !lit.folder_ids.contains(&"trash".to_string()))
                .collect()
        } else if folder_id == "uncategorized" {
            literatures
                .iter()
                .filter(|lit| lit.folder_ids.is_empty())
                .collect()
        } else {
            literatures
                .iter()
                .filter(|lit| lit.folder_ids.contains(folder_id))
                .collect()
        };
        debug!("数据层: 筛选文件夹 id={folder_id} => {} 条", v.len());
        v
    } else {
        debug!("数据层: 无筛选条件，返回空");
        Vec::new()
    };

    results = sort_literatures(results, sort_field, sort_order);
    results
}

/// 获取当前订阅源列表
#[must_use]
pub fn get_feed_items<'a>(
    feed_items: &'a [Arc<FeedItem>],
    selected_feed_id: &Option<String>,
) -> Vec<&'a Arc<FeedItem>> {
    let result = if let Some(feed_id) = selected_feed_id.as_ref() {
        if feed_id == "all_subs" {
            feed_items.iter().collect()
        } else if feed_id == "unread" {
            feed_items.iter().filter(|s| !s.is_read).collect()
        } else {
            feed_items
                .iter()
                .filter(|s| s.feed_id == *feed_id)
                .collect()
        }
    } else {
        Vec::new()
    };
    debug!(
        "数据层: 筛选订阅源 feed_id={:?} => {} 条 (共 {} 条)",
        selected_feed_id,
        result.len(),
        feed_items.len()
    );
    result
}

/// 搜索文献（在当前筛选范围内）
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn search_literatures<'a>(
    literatures: &'a [Arc<Literature>],
    _folders: &[Arc<Folder>],
    tags: &[(Arc<Tag>, usize)],
    selected_folder_id: &Option<String>,
    selected_tag_id: &Option<String>,
    sort_field: SortField,
    sort_order: SortOrder,
    advanced_search_query: &AdvancedSearchQuery,
    query: &str,
) -> Vec<&'a Arc<Literature>> {
    let current_items = get_folder_literatures(
        literatures,
        tags,
        selected_folder_id,
        selected_tag_id,
        sort_field,
        sort_order,
    );
    debug!(
        "数据层: 搜索 query='{query}', advanced={:?}, 基础范围 {} 条",
        advanced_search_query,
        current_items.len()
    );
    let base_results = SearchEngine::search(query, current_items);
    debug!("数据层: 基础搜索命中 {} 条", base_results.len());

    let results = if advanced_search_query.is_empty() {
        base_results
    } else {
        let advanced_results = SearchEngine::advanced_search(advanced_search_query, base_results);
        debug!("数据层: 高级搜索命中 {} 条", advanced_results.len());
        advanced_results
    };

    sort_literatures(results, sort_field, sort_order)
}
