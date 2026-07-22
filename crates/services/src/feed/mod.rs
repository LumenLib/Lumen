//! 订阅 / 抓取域（feed）
//!
//! RSS 订阅（`feed`）与网页抓取解析（`fetcher`）。仅做 CRUD，
//! 碰数据库是本职，不感知同步。
//!
//! 解耦约定：域方法不再收 `&MainApp`，改为收 `&Database` / `Arc<Database>`；
//! 后台刷新循环通过注入 `notify` 回调上行业务通知，自身不感知 UI / 同步。

pub mod feed;
pub mod fetcher;

pub use feed::{FeedService, SubscriptionRefreshResult};
pub use fetcher::FetcherService;
