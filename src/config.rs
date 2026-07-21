//! 配置模块（重导出 shim）
//!
//! 配置类型与平台相关自由函数已迁移至 `models::config`，这里仅做重导出，
//! 以保持既有 `config::AppConfig` / `config::get_app_root_dir` / `config::apply_proxy_config`
//! 等调用路径继续可用，避免大规模改动调用处。

pub use models::config::*;
