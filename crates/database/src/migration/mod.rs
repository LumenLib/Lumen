pub mod remote;
pub mod utils;
pub mod v010;

use anyhow::Result;
use log::{debug, info};
use rusqlite::{Connection, params};
use std::collections::HashSet;
use std::path::Path;

/// 一个数据库迁移
pub struct Migration {
    /// 版本号（如 "v010"、"v020"）
    pub version: &'static str,
    /// 中文描述
    pub description: &'static str,
    /// 迁移函数（必须幂等）
    pub up: fn(&Connection) -> Result<()>,
}

/// 执行所有未应用的迁移
///
/// 流程：
/// 1. 确保 `schema_version` 表存在
/// 2. 查询已应用版本
/// 3. 有 pending 迁移时先备份再逐条执行
pub fn run_migrations(conn: &Connection, db_path: &Path, registry: &[Migration]) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version TEXT PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        )",
        [],
    )?;

    let applied: HashSet<String> = conn
        .prepare("SELECT version FROM schema_version")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()?;

    let pending: Vec<&Migration> = registry.iter().filter(|m| !applied.contains(m.version)).collect();

    if pending.is_empty() {
        debug!("迁移: 无需执行新迁移");
        return Ok(());
    }

    info!("迁移: 检测到 {} 个待执行迁移，开始备份数据库", pending.len());

    utils::backup_database(conn, db_path)?;

    for migration in &pending {
        debug!("迁移: 正在执行 {} - {}", migration.version, migration.description);
        (migration.up)(conn)?;
        conn.execute(
            "INSERT INTO schema_version (version, description, applied_at) VALUES (?1, ?2, ?3)",
            params![migration.version, migration.description, chrono::Utc::now().timestamp()],
        )?;
        info!("迁移: {} - {} 已完成", migration.version, migration.description);
    }

    Ok(())
}

/// 所有已注册的数据库迁移（按版本升序）
///
/// 每发布一个有数据库变更的版本，在此添加对应的 `migration()` 调用。
pub fn all_migrations() -> Vec<Migration> {
    vec![
        v010::migration(),
    ]
}
