//! 应用层运行时状态（与 GPUI 强耦合），保留在 lumen 主 crate 内。
//!
//! 服务层逻辑已全部下沉到 `services` crate（纯逻辑，不依赖 GPUI）；此处仅收纳
//! 与 GPUI 强耦合的运行时模块：
//! - `data`：`DataStore` 实体（持有 `Arc<Database>` + 全量领域缓存）
//! - `config`：`ConfigStore` 全局（GPUI `Global`，配置）
//! - `ui`：`UiState` 全局（GPUI `Global`，UI 选中/排序/视图状态）
//! - `theme`：主题运行时全局态（`SurfaceState` / `ThemeLoaderState`）+ 便捷函数
//!
//! UI 代码直接引用 `services` crate（如 `services::app::MainApp`），不再经本模块中转。
//! 跨线程通知消息 `RefreshMsg` 直接引用 `services::notify::RefreshMsg`，本模块不再重导出。

pub mod config;
pub mod data;
pub mod theme;
pub mod ui;
