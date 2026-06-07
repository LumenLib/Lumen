use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use database::Database;
use gpui::{Context, Entity, EventEmitter};
use log::{debug, error, info, warn};
use models::{Feed, FeedItem, Folder, Literature, Tag};

/// 跨线程通知消息 —— 桥接 tokio 上下文 → GPUI 主循环（DataStore 刷新）
///
/// 设计意图：service 层在 tokio 中写 DB 后，无法直接调用 `Entity::update`，
#[derive(Clone, Debug)]
pub enum RefreshMsg {
    /// 领域数据变更（触发 DataStore.refresh_from_db）
    DataChanged,
    /// UI 状态变更（仅触发 cx.notify，无需刷新 DB）
    UiChanged,
}

/// 领域事件 —— DataStore 的数据变更通知
#[derive(Clone, Debug)]
pub enum DataStoreEvent {
    /// 粗粒度 catch-all
    DataChanged,
}

impl EventEmitter<DataStoreEvent> for DataStore {}

/// DataStore —— GPUI Entity，持有数据库引用 + 全量领域缓存
///
/// 所有写操作通过 MainApp → service layer → `app.db.*` → RefreshMsg 桥 → refresh_from_db 完成。
/// DataStore 不直接提供 CRUD 方法，只做只读缓存 + 全量重载。
pub struct DataStore {
    pub db: Arc<Database>,

    /// 全量缓存（Arc 共享，子视图持有 Arc 避免深克隆）
    pub literatures: Vec<Arc<Literature>>,
    pub folders: Vec<Arc<Folder>>,
    pub tags: Vec<(Arc<Tag>, usize)>,
    pub feeds: Vec<Arc<Feed>>,
    pub feed_items: Vec<Arc<FeedItem>>,
}

impl DataStore {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            literatures: Vec::new(),
            folders: Vec::new(),
            tags: Vec::new(),
            feeds: Vec::new(),
            feed_items: Vec::new(),
        }
    }

    /// 从数据库重载所有缓存，发射 DataChanged
    pub fn refresh_from_db(&mut self, cx: &mut Context<Self>) -> Result<()> {
        info!("[DataStore] 开始从数据库刷新全量缓存...");

        self.literatures = self
            .db
            .get_all_literatures()
            .map_err(|e| {
                error!("[DataStore] 获取文献列表失败: {e}");
                e
            })?
            .into_iter()
            .map(Arc::new)
            .collect();
        self.folders = self
            .db
            .get_all_folders()
            .map_err(|e| {
                error!("[DataStore] 获取文件夹列表失败: {e}");
                e
            })?
            .into_iter()
            .map(Arc::new)
            .collect();
        self.tags = self
            .db
            .get_all_tags_with_counts()
            .map_err(|e| {
                warn!("[DataStore] 获取标签列表失败: {e}");
                e
            })?
            .into_iter()
            .map(|(t, n)| (Arc::new(t), n))
            .collect();
        self.feeds = self
            .db
            .get_all_feeds()
            .map_err(|e| {
                error!("[DataStore] 获取订阅源列表失败: {e}");
                e
            })?
            .into_iter()
            .map(Arc::new)
            .collect();

        let mut all_items: Vec<Arc<FeedItem>> = Vec::new();
        for feed in &self.feeds {
            if feed.id != "all_subs"
                && feed.id != "unread"
                && let Ok(items) = self.db.get_feed_items_by_feed(&feed.id)
            {
                all_items.extend(items.into_iter().map(Arc::new));
            }
        }
        self.feed_items = all_items;

        debug!(
            "[DataStore] refresh_from_db: literatures={}, folders={}, tags={}, feeds={}, feed_items={}",
            self.literatures.len(),
            self.folders.len(),
            self.tags.len(),
            self.feeds.len(),
            self.feed_items.len()
        );

        self.update_folder_counts();
        self.update_feed_counts();

        cx.emit(DataStoreEvent::DataChanged);
        cx.notify();
        info!("[DataStore] 全量缓存刷新完成");
        Ok(())
    }

    /// 遍历 `literatures` 统计每个文件夹的文献数量
    fn update_folder_counts(&mut self) {
        let has_all = self.folders.iter().any(|f| f.id == "all");
        let has_uncat = self.folders.iter().any(|f| f.id == "uncategorized");
        let has_trash = self.folders.iter().any(|f| f.id == "trash");
        debug!(
            "[DataStore::update_folder_counts] 开始计数: literatures={}, folders={}, has_all={}, has_uncat={}, has_trash={}",
            self.literatures.len(),
            self.folders.len(),
            has_all,
            has_uncat,
            has_trash
        );

        for folder in &mut self.folders {
            Arc::make_mut(folder).literature_count = 0;
        }

        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut all_count: usize = 0;
        let mut uncategorized_count: usize = 0;

        for lit in &self.literatures {
            if lit.folder_ids.contains(&"trash".to_string()) {
                *counts.entry("trash".to_string()).or_insert(0) += 1;
                continue;
            }
            all_count += 1;
            if lit.folder_ids.is_empty() {
                uncategorized_count += 1;
            } else {
                for folder_id in &lit.folder_ids {
                    *counts.entry(folder_id.clone()).or_insert(0) += 1;
                }
            }
        }

        for folder in &mut self.folders {
            if let Some(&count) = counts.get(&folder.id) {
                Arc::make_mut(folder).literature_count = count;
            }
        }

        if let Some(all) = self.folders.iter_mut().find(|f| f.id == "all") {
            Arc::make_mut(all).literature_count = all_count;
        }
        if let Some(uncat) = self.folders.iter_mut().find(|f| f.id == "uncategorized") {
            Arc::make_mut(uncat).literature_count = uncategorized_count;
        }

        // 日志：输出每个文件夹的最终计数
        for f in &self.folders {
            debug!(
                "[DataStore::update_folder_counts]   folder '{}' ({}): literature_count={}",
                f.name, f.id, f.literature_count
            );
        }
    }

    fn update_feed_counts(&mut self) {
        // 统计每个 feed 的总数和未读数
        let mut total_map: HashMap<String, usize> = HashMap::new();
        let mut unread_map: HashMap<String, usize> = HashMap::new();
        let mut all_total: usize = 0;
        let mut all_unread: usize = 0;

        for item in &self.feed_items {
            if item.is_deleted {
                continue;
            }
            all_total += 1;
            if !item.is_read {
                all_unread += 1;
            }
            *total_map.entry(item.feed_id.clone()).or_insert(0) += 1;
            if !item.is_read {
                *unread_map.entry(item.feed_id.clone()).or_insert(0) += 1;
            }
        }

        for feed in &mut self.feeds {
            let f = Arc::make_mut(feed);
            if f.id == "all_subs" {
                f.total_count = all_total;
                f.unread_count = all_unread;
            } else if f.id == "unread" {
                f.total_count = all_unread;
                f.unread_count = all_unread;
            } else {
                f.total_count = total_map.get(&f.id).copied().unwrap_or(0);
                f.unread_count = unread_map.get(&f.id).copied().unwrap_or(0);
            }
        }

        debug!("[DataStore::update_feed_counts] all_total={all_total}, all_unread={all_unread}");
        for f in &self.feeds {
            debug!(
                "[DataStore::update_feed_counts]   feed '{}' ({}): total={}, unread={}",
                f.name, f.id, f.total_count, f.unread_count
            );
        }
    }
}

pub type DataStoreEntity = Entity<DataStore>;
