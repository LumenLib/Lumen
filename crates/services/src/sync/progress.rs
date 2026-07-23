//! 同步状态类型（跨线程共享，从 Tokio 写入，UI 读取）
//!
//! 原定义于 `lumen` 的 `MainApp`，随同步编排一并下沉到 `services::sync`，
//! 使 `MainApp` 只持有 `Arc<Mutex<SyncStateInner>>` 引用而不再定义该类型。

use models::Literature;

/// 同步状态
#[derive(Debug, Clone, PartialEq)]
pub enum SyncStatus {
    Idle,
    Syncing,
    Conflict(Vec<Literature>),
    Error(String),
}

/// 同步状态（跨线程共享）
///
/// - `sync_status` / `attachment_sync_status`：由 `engine`/`metadata`/`attachments` 在异步任务中写入。
/// - `sync_conflict_groups`：仅在 UI 侧解析冲突后写入/读取，同步逻辑不直接触碰，保持 UI 自治。
#[derive(Debug, Clone)]
pub struct SyncStateInner {
    pub sync_status: SyncStatus,
    pub attachment_sync_status: SyncStatus,
    pub sync_conflict_groups: Option<Vec<Vec<Literature>>>,
}

impl SyncStateInner {
    pub fn new() -> Self {
        Self {
            sync_status: SyncStatus::Idle,
            attachment_sync_status: SyncStatus::Idle,
            sync_conflict_groups: None,
        }
    }
}

impl Default for SyncStateInner {
    fn default() -> Self {
        Self::new()
    }
}
