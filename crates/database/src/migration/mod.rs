pub mod remote;
pub mod utils;
pub mod v011;
pub mod v012;
pub mod v013;

use anyhow::Result;
use log::{debug, info};
use rusqlite::{Connection, params};
use std::collections::HashSet;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Migration {
    pub version: &'static str,
    pub description: &'static str,
    pub up: fn(&Connection) -> Result<()>,
}

/// 在当前 SQLite 数据库上执行所有待运行的迁移
///
/// 流程：
/// 1. 创建 `schema_version` 表（首次运行时）
/// 2. 对比已应用的版本，筛选出待执行的迁移
/// 3. 执行前先通过 `utils::backup_database` 备份数据库
/// 4. 按版本顺序逐个执行，每个迁移成功后在 `schema_version` 中记录
/// 5. 任一迁移失败则中止（数据仍可通过备份恢复）
pub fn run_migrations(conn: &Connection, db_path: &Path, migrations: &[Migration]) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version TEXT PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        )",
        [],
    )?;

    let mut stmt = conn.prepare("SELECT version FROM schema_version")?;
    let applied: HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()?;

    let pending: Vec<&Migration> = migrations
        .iter()
        .filter(|m| !applied.contains(m.version))
        .collect();

    if pending.is_empty() {
        debug!("数据库架构已是最新");
        return Ok(());
    }

    info!("检测到 {} 个待执行迁移, 正在备份数据库...", pending.len());
    utils::backup_database(conn, db_path)?;

    for m in &pending {
        debug!("正在执行迁移 {}: {}", m.version, m.description);
        (m.up)(conn)?;
        conn.execute(
            "INSERT INTO schema_version (version, description, applied_at) VALUES (?1, ?2, ?3)",
            params![
                m.version,
                m.description,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64
            ],
        )?;
        info!("迁移 {}: {} 已完成", m.version, m.description);
    }

    Ok(())
}

/// 所有已注册的数据库迁移（按版本升序）
///
/// 每发布一个有数据库变更的版本，在此添加对应的 `migration()` 调用。
pub fn all_migrations() -> Vec<Migration> {
    vec![
        v011::migration(),
        v012::migration(),
        v013::migration(),
    ]
}
