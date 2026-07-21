//! Lumen 侧 GPUI 耦合模块（真身，留 lumen）。
//!
//! 服务层逻辑已全部下沉 `services` crate；此处仅保留与 GPUI 强耦合的运行时模块：
//! - `data_store`：`DataStore` 实体 + `RefreshMsg`（从 `services::notify` 重导出，供 UI 订阅）
//! - `ui_state`：`UiState` 全局（GPUI `Global`）
//!
//! UI 代码直接引用 `services` crate（如 `services::app::MainApp`），不再经本模块中转。

pub mod data_store;
pub mod ui_state;
