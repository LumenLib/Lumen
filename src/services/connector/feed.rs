use crate::RUNTIME;
use crate::services::MainApp;
use anyhow::{Error, Result, anyhow};
use log::{debug, error, info, warn};
/// 数据库操作单例管理器
///
/// 负责协调持久化存储与内存数据的同步
use models::{Feed, FeedItem};
use parser::{ElsevierSubscriptionParser, IeeeSubscriptionParser, RssSubscriptionParser};
use reqwest::Client;
use std::sync::Arc;

pub struct FeedService;

impl FeedService {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FeedService {
    fn default() -> Self {
        Self::new()
    }
}

impl FeedService {
    // --- Subscription Operations ---

    pub fn save_feed(&self, app: &MainApp, feed: Feed) -> Result<()> {
        info!(
            "数据库管理: 正在保存订阅源: '{}' (ID: {})",
            feed.name, feed.id
        );
        app.db.insert_feed(&feed)?;
        Ok(())
    }

    pub fn delete_feed(&self, app: &MainApp, id: &str) -> Result<()> {
        info!("数据库管理: 正在删除订阅源 (ID: {id})");
        app.db.delete_feed(id)?;
        app.db.delete_items_by_feed(id)?;
        info!("数据库管理: 订阅源及其明细已从数据库删除");
        Ok(())
    }

    pub fn update_feed_item_read_status(
        &self,
        app: &MainApp,
        id: &str,
        is_read: bool,
    ) -> Result<()> {
        debug!("数据库管理: 更新订阅项阅读状态 (ID: {id}, is_read: {is_read})");
        app.db.update_feed_item_read_status(id, is_read)?;
        Ok(())
    }

    pub fn update_feed_item_added_status(
        &self,
        app: &MainApp,
        id: &str,
        is_added: bool,
    ) -> Result<()> {
        debug!("数据库管理: 更新订阅项添加状态 (ID: {id}, is_added: {is_added})");
        app.db.update_feed_item_added_status(id, is_added)?;
        Ok(())
    }

    pub fn delete_feed_item(&self, app: &MainApp, id: &str) -> Result<()> {
        info!("数据库管理: 正在删除订阅项 (ID: {id})");
        app.db.delete_feed_item(id)?;
        Ok(())
    }

    pub fn add_feed_item(&self, app: &MainApp, item: FeedItem) -> Result<()> {
        debug!(
            "数据库管理: 正在添加订阅项: '{}' (feed_id: {})",
            item.title, item.feed_id
        );
        if let Err(e) = app.db.insert_feed_item(&item) {
            warn!("数据库管理: 添加订阅项 '{}' 失败: {e}", item.title);
            return Err(e.into());
        }
        Ok(())
    }
}

impl FeedService {
    /// 刷新特定订阅源的内容
    pub async fn refresh_feed(&self, app: Arc<MainApp>, feed_id: String) -> Result<()> {
        info!("订阅管理: 正在请求刷新订阅源 (ID: {feed_id})");
        let app_clone = app.clone();
        let feed_id_clone = feed_id.clone();

        let result = RUNTIME.spawn(async move {
            let app = app_clone;
            let feed_id = feed_id_clone;

            let (feed_name, feed_url, last_update_time) = {
                let feed = match app.db.get_feed(&feed_id) {
                    Ok(Some(feed)) => Some(feed),
                    Ok(None) => {
                        warn!("订阅管理: 数据库未找到订阅源 (ID: {feed_id})");
                        None
                    }
                    Err(e) => {
                        warn!("订阅管理: 查询订阅源失败 (ID: {feed_id}): {e}");
                        None
                    }
                };
                (
                    feed.as_ref().map(|f| f.name.clone()),
                    feed.as_ref().and_then(|f| f.url.clone()),
                    feed.as_ref().and_then(|f| f.last_updated_at.clone())
                )
            };

            if let Some(url) = feed_url {
                let name = feed_name.unwrap_or_else(|| "未知订阅源".to_string());
                info!("订阅管理: 正在从 {url} 获取订阅内容 [{name}]");
                let client = Client::new();
                let response = client
                    .get(&url)
                    .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                    .send()
                    .await
                    .map_err(|e| {
                        error!("订阅管理: HTTP 请求失败 ({url}): {e}");
                        e
                    })?;

                let status = response.status();
                if !status.is_success() {
                    warn!("订阅管理: HTTP 非成功状态码 {status} ({url})");
                }
                let content = response.text().await.map_err(|e| {
                    error!("订阅管理: 读取响应体失败 (status: {status}): {e}");
                    e
                })?;
                debug!("订阅管理: 成功下载内容，长度 {} 字节", content.len());

                // 根据内容或 URL 启发式判断解析器
                let (items, source_update_time) = if url.contains("ieee.org") {
                    debug!("订阅管理: 使用 IEEE 解析器");
                    IeeeSubscriptionParser::new().parse_list(&content, &feed_id)?
                } else if url.contains("sciencedirect.com") {
                    debug!("订阅管理: 使用 Elsevier 解析器");
                    ElsevierSubscriptionParser::parse(&content, &feed_id)?
                } else {
                    debug!("订阅管理: 使用通用 RSS 解析器 (支持 DC/PRISM/Atom)");
                    RssSubscriptionParser::parse(&content, &feed_id)?
                };

                // 比较更新时间，如果一致则跳过
                if let (Some(last), Some(current)) = (&last_update_time, &source_update_time)
                    && last == current {
                        info!("订阅管理: 订阅源[{name}]内容未更新 (时间戳一致: {current}), 跳过数据库写入");
                        return Ok(());
                }

                info!("订阅管理: 成功解析出 {} 条订阅项, 更新时间: {:?}", items.len(), source_update_time);

                // 调试日志：检查解析出的 DOI
                for (i, item) in items.iter().enumerate().take(3) {
                    debug!("订阅管理: 解析示例 [{}]: title={}, doi={:?}", i, item.title, item.doi);
                }

                // 将项目存入数据库
                let mut added_count = 0;
                let mut failed_count = 0;
                for item in items {
                    match app.feed_service.add_feed_item(&app, item) {
                        Ok(()) => added_count += 1,
                        Err(e) => {
                            warn!("订阅管理: 保存订阅项失败: {e}");
                            failed_count += 1;
                        }
                    }
                }
                if failed_count > 0 {
                    warn!("订阅管理: [{name}] 保存完成，成功 {added_count} 条，失败 {failed_count} 条");
                }
                debug!("订阅管理: 已将 {added_count} 条新项目写入数据库");

                // 更新 Feed 的最后更新时间
                if let Some(new_time) = source_update_time {
                    let feed = app.db.get_feed(&feed_id).ok().flatten();
                    if let Some(mut f) = feed {
                        f.last_updated_at = Some(new_time.clone());
                        if let Err(e) = app.feed_service.save_feed(&app, f) {
                            warn!("订阅管理: 更新订阅源最后同步时间失败: {e}");
                        } else {
                            debug!("订阅管理: 订阅源 [{name}] 最后同步时间已更新为 {new_time}");
                        }
                    }
                }

                // 通知 UI 刷新
                app.notify_data_changed();
            } else {
                warn!("订阅管理: 未找到 ID 为 {feed_id} 的订阅源 URL");
            }
            Ok::<(), Error>(())
        }).await;

        match result {
            Ok(inner_result) => inner_result,
            Err(join_err) => {
                error!("订阅管理: 刷新任务崩溃: {join_err}");
                Err(anyhow!("Task panicked or cancelled: {join_err}"))
            }
        }
    }

    /// 刷新所有订阅源
    pub async fn refresh_all(&self, app: Arc<MainApp>) -> Result<()> {
        info!("订阅管理: 正在执行全量订阅刷新...");
        let feed_ids: Vec<String> = app
            .db
            .get_all_feeds()
            .unwrap_or_default()
            .into_iter()
            .filter(|f| f.id != "all_subs" && f.id != "unread")
            .map(|f| f.id)
            .collect();

        let total = feed_ids.len();
        debug!("订阅管理: 共需刷新 {total} 个订阅源");
        for (i, id) in feed_ids.into_iter().enumerate() {
            debug!("订阅管理: 正在刷新第 {}/{} 个订阅源", i + 1, total);
            if let Err(e) = self.refresh_feed(app.clone(), id.clone()).await {
                warn!("订阅管理: 刷新订阅源 (ID: {id}) 失败: {e}");
            }
        }

        info!("订阅管理: 全量订阅刷新完成");
        Ok(())
    }

    /// 启动后台自动更新任务
    pub fn start_background_loop(self: Arc<Self>, app: Arc<MainApp>) {
        let manager = self.clone();
        RUNTIME.spawn(async move {
            info!("订阅后台更新循环已启动");
            // 每 10 分钟检查一次是否有需要更新的订阅源
            let mut heartbeat = tokio::time::interval(tokio::time::Duration::from_secs(600));
            loop {
                heartbeat.tick().await;

                let feeds = match app.db.get_all_feeds() {
                    Ok(feeds) => feeds,
                    Err(e) => {
                        warn!("订阅管理: 后台更新获取订阅源列表失败: {e}");
                        continue;
                    }
                };

                for feed in feeds {
                    // 跳过内置的虚拟节点
                    if feed.id == "all_subs" || feed.id == "unread" {
                        continue;
                    }

                    let interval_hours = i64::from(feed.update_interval);
                    if interval_hours == 0 {
                        debug!("订阅管理: 订阅源[{}]更新间隔为0，跳过自动更新", feed.name);
                        continue;
                    }

                    let should_update = match &feed.last_updated_at {
                        Some(last_time_str) => {
                            match chrono::NaiveDateTime::parse_from_str(
                                last_time_str,
                                "%Y-%m-%d %H:%M:%S",
                            ) {
                                Ok(last_time) => {
                                    let now = chrono::Local::now().naive_local();
                                    let elapsed = now - last_time;
                                    elapsed.num_hours() >= interval_hours
                                }
                                Err(_) => true, // 解析失败则尝试更新
                            }
                        }
                        None => true, // 从未更新过
                    };

                    if should_update {
                        info!(
                            "订阅源[{}]已到达更新周期({}小时)，准备执行后台更新...",
                            feed.name, interval_hours
                        );
                        if let Err(e) = manager.refresh_feed(app.clone(), feed.id).await {
                            error!("后台自动更新订阅源[{}]失败: {}", feed.name, e);
                        }
                    }
                }
            }
        });
    }
}
