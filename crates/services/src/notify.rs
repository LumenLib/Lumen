//! 跨线程通知消息 —— 桥接 tokio 上下文 → GPUI 主循环（DataStore 刷新）。
//!
//! 纯数据枚举，不依赖 `gpui`，故置于服务层 crate 供 `app`（组合根）与
//! 各服务模块构造通知闭包时使用；lumen 侧的 `DataStore`（GPUI `Entity`）
//! 订阅同名广播通道并据此刷新。

/// service 层在 tokio 中写 DB 后，无法直接调用 `Entity::update`，
/// 只能通过广播此消息让 GPUI 主循环完成 UI 刷新。
#[derive(Clone, Debug)]
pub enum RefreshMsg {
    /// 领域数据变更（触发 DataStore.refresh_from_db）
    DataChanged,
    /// UI 状态变更（仅触发 cx.notify，无需刷新 DB）
    UiChanged,
}
