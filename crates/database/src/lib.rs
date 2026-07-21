//! 数据库层
//!
//! 本地 SQLite CRUD + MySQL 远程同步。

pub mod local_state;
pub mod migration;
pub mod mysql;
pub mod sqlite;

pub use local_state::LocalStateManager;
pub use mysql::MySqlManager;
pub use sqlite::Database;

/// 数据库配置（类型定义归属 models 层，这里仅做重导出以兼容既有 `database::DatabaseConfig` / `crate::DatabaseConfig` 调用路径）
pub use models::DatabaseConfig;
