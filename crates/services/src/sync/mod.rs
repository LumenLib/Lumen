//! 同步编排域（sync）—— 独立模块，只调用核心
//!
//! - `engine`：自动同步循环与上传/元数据/下载三段式编排（`SyncService`）
//! - `metadata`：本地 DB <-> 云 MySQL（`SQLSyncService`，收 `database/mysql` 的 push/pull）
//! - `attachments`：本地文件 <-> 云文件（`FileSyncService`，收 `crates/file` 的 backend）
//! - `progress`：跨线程共享的同步状态类型（`SyncStateInner` / `SyncStatus`）
//!
//! 解耦要点：本模块不再依赖 `MainApp`。`MainApp` 在构造 `SyncService` 时注入
//! `sync_state: Arc<Mutex<SyncStateInner>>` 与两个 `'static` 通知闭包
//!（`notify_data` = 仅 `DataChanged`；`notify_ui` = `UiChanged`），由闭包桥接 GPUI 刷新。
//!
//! 依赖 `database` + `sync`（文件传输）；上游 UI 通过 `services::app` 组合根接线，
//! 核心不反向依赖本模块。

pub mod attachments;
pub mod conflict;
pub mod engine;
pub mod metadata;
pub mod progress;
pub mod remote;

pub use engine::SyncService;
pub use progress::{SyncStateInner, SyncStatus};
