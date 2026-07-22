use anyhow::{Error, Result, anyhow};
use database::Database;
use log::{debug, error, info, warn};
/// 数据库操作单例管理器
///
/// 负责协调持久化存储与内存数据的同步
use models::{Feed, FeedItem};
use parser::{ElsevierSubscriptionParser, IeeeSubscriptionParser, NatureSubscriptionParser};
use reqwest::Client;
use std::sync::Arc;

use crate::runtime::RUNTIME;

/// 单个订阅刷新完成后的结果，用于驱动 UI 逐条通知。
///
/// `refresh_all` 在循环里对每个订阅调用一次回调（成功/失败各一次），
/// 失败不中断后续订阅；具体通知由调用方注入的闭包负责（services 层不感知 UI）。
#[derive(Debug, Clone)]
pub enum SubscriptionRefreshResult {
    Ok { name: String },
    Err { name: String, error: String },
}

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

    pub fn save_feed(&self, db: &Database, feed: Feed) -> Result<()> {
        info!(
            "数据库管理: 正在保存订阅源: '{}' (ID: {})",
            feed.name, feed.id
        );
        db.insert_feed(&feed)?;
        Ok(())
    }

    pub fn delete_feed(&self, db: &Database, id: &str) -> Result<()> {
        info!("数据库管理: 正在删除订阅源 (ID: {id})");
        db.delete_feed(id)?;
        db.delete_items_by_feed(id)?;
        info!("数据库管理: 订阅源及其明细已从数据库删除");
        Ok(())
    }

    pub fn update_feed_item_read_status(
        &self,
        db: &Database,
        id: &str,
        is_read: bool,
    ) -> Result<()> {
        debug!("数据库管理: 更新订阅项阅读状态 (ID: {id}, is_read: {is_read})");
        db.update_feed_item_read_status(id, is_read)?;
        Ok(())
    }

    pub fn update_feed_item_added_status(
        &self,
        db: &Database,
        id: &str,
        is_added: bool,
    ) -> Result<()> {
        debug!("数据库管理: 更新订阅项添加状态 (ID: {id}, is_added: {is_added})");
        db.update_feed_item_added_status(id, is_added)?;
        Ok(())
    }

    pub fn delete_feed_item(&self, db: &Database, id: &str) -> Result<()> {
        info!("数据库管理: 正在删除订阅项 (ID: {id})");
        db.delete_feed_item(id)?;
        Ok(())
    }

    pub fn add_feed_item(&self, db: &Database, item: FeedItem) -> Result<()> {
        debug!(
            "数据库管理: 正在添加订阅项: '{}' (feed_id: {})",
            item.title, item.feed_id
        );
        if let Err(e) = db.insert_feed_item(&item) {
            warn!("数据库管理: 添加订阅项 '{}' 失败: {e}", item.title);
            return Err(e.into());
        }
        Ok(())
    }
}

impl FeedService {
    /// 刷新特定订阅源的内容
    pub async fn refresh_feed(&self, db: Arc<Database>, feed_id: String) -> Result<()> {
        info!("订阅管理: 正在请求刷新订阅源 (ID: {feed_id})");
        let db_clone = db.clone();
        let feed_id_clone = feed_id.clone();

        let result = RUNTIME
            .spawn(async move {
                let db = db_clone;
                let feed_id = feed_id_clone;

                let (feed_name, feed_url, last_update_time) = {
                    let feed = match db.get_feed(&feed_id) {
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
                        feed.as_ref().and_then(|f| f.last_updated_at.clone()),
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

                    // 按 URL 精确路由到对应的专用解析器（Nature / IEEE / Elsevier），
                    // 不再保留通用兜底：非三类的源直接报错，不做额外适配。
                    let (items, channel_title, source_update_time) = if url.contains("nature.com") {
                        debug!("订阅管理: 使用 Nature 解析器");
                        NatureSubscriptionParser::parse(&content, &feed_id)?
                    } else if url.contains("ieee.org") {
                        debug!("订阅管理: 使用 IEEE 解析器");
                        IeeeSubscriptionParser::new().parse_list(&content, &feed_id)?
                    } else if url.contains("sciencedirect.com") {
                        debug!("订阅管理: 使用 Elsevier 解析器");
                        ElsevierSubscriptionParser::parse(&content, &feed_id)?
                    } else {
                        return Err(anyhow!("不支持的订阅源（仅支持 Nature/IEEE/Elsevier）：{url}"));
                    };

                    // 比较更新时间，如果一致则跳过
                    if let (Some(last), Some(current)) = (&last_update_time, &source_update_time)
                        && last == current
                    {
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
                        match FeedService::new().add_feed_item(&db, item) {
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

                    // 更新 Feed 的最后更新时间与频道标题
                    if let Some(new_time) = source_update_time {
                        let feed = db.get_feed(&feed_id).ok().flatten();
                        if let Some(mut f) = feed {
                            f.last_updated_at = Some(new_time.clone());
                            // 频道标题（如 "Nature Communications"）来自解析器，
                            // 优先展示；为空时保留已有值。
                            if let Some(t) = channel_title.clone().filter(|s| !s.trim().is_empty()) {
                                f.title = Some(t);
                            }
                            if let Err(e) = FeedService::new().save_feed(&db, f) {
                                warn!("订阅管理: 更新订阅源最后同步时间失败: {e}");
                            } else {
                                debug!("订阅管理: 订阅源 [{name}] 最后同步时间已更新为 {new_time}");
                            }
                        }
                    }
                } else {
                    warn!("订阅管理: 未找到 ID 为 {feed_id} 的订阅源 URL");
                }
                Ok::<(), Error>(())
            })
            .await;

        match result {
            Ok(inner_result) => inner_result,
            Err(join_err) => {
                error!("订阅管理: 刷新任务崩溃: {join_err}");
                Err(anyhow!("Task panicked or cancelled: {join_err}"))
            }
        }
    }

    /// 刷新所有订阅源
    ///
    /// 对每个真实订阅依次 `refresh_feed`：完成后通过 `on_result` 回调上报结果
    /// （成功/失败各一次）；单个失败时仅 `warn` 并继续下一个，不中断整轮。
    /// `on_result` 由调用方注入，用于把结果传回 UI 弹通知（services 层不感知 UI）。
    pub async fn refresh_all(
        &self,
        db: Arc<Database>,
        on_result: Arc<dyn Fn(SubscriptionRefreshResult) + Send + Sync>,
    ) -> Result<()> {
        info!("订阅管理: 正在执行全量订阅刷新...");
        let feed_ids: Vec<String> = db
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
            let name = db
                .get_feed(&id)
                .ok()
                .flatten()
                .map(|f| f.name)
                .unwrap_or_else(|| id.clone());
            match self.refresh_feed(db.clone(), id.clone()).await {
                Ok(()) => {
                    debug!("订阅管理: 订阅源 [{name}] 刷新成功");
                    on_result(SubscriptionRefreshResult::Ok { name });
                }
                Err(e) => {
                    warn!("订阅管理: 刷新订阅源 [{name}] (ID: {id}) 失败: {e}");
                    on_result(SubscriptionRefreshResult::Err {
                        name,
                        error: e.to_string(),
                    });
                }
            }
        }

        info!("订阅管理: 全量订阅刷新完成");
        Ok(())
    }

    /// 启动后台自动更新任务
    ///
    /// `notify` 由组合根（MainApp）注入，用于刷新后触发 UI 重绘与同步请求；
    /// 域服务本身不感知 UI / 同步（架构红线）。
    pub fn start_background_loop(
        self: Arc<Self>,
        db: Arc<Database>,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) {
        let manager = self.clone();
        RUNTIME.spawn(async move {
            info!("订阅后台更新循环已启动");
            // 每 10 分钟检查一次是否有需要更新的订阅源
            let mut heartbeat = tokio::time::interval(tokio::time::Duration::from_secs(600));
            loop {
                heartbeat.tick().await;

                let feeds = match db.get_all_feeds() {
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
                        if let Err(e) = manager.refresh_feed(db.clone(), feed.id).await {
                            error!("后台自动更新订阅源[{}]失败: {}", feed.name, e);
                        } else {
                            notify();
                        }
                    }
                }
            }
        });
    }
}
