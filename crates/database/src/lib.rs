//! 数据库层
//!
//! 本地 SQLite CRUD + MySQL 远程同步。

pub mod ccf_data;
pub mod local_state;
pub mod mysql;
pub mod sqlite;

pub mod constructors;

pub use local_state::LocalStateManager;
pub use mysql::MySqlManager;
pub use sqlite::Database;

/// 数据库配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatabaseConfig {
    pub use_remote: bool,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub use_ssl: bool,
}
