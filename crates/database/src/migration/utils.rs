use anyhow::{Context, Result};
use log::info;
use rusqlite::{Connection, DatabaseName, params};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// 检查表是否存在
pub fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// 获取表的所有列名
pub fn get_column_names(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&sql)?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(names)
}

/// 检查列是否存在
pub fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let names = get_column_names(conn, table)?;
    Ok(names.iter().any(|n| n == column))
}

/// 添加列（如果不存在）
pub fn add_column(conn: &Connection, table: &str, name: &str, decl: &str) -> Result<()> {
    if !column_exists(conn, table, name)? {
        let sql = format!("ALTER TABLE {table} ADD COLUMN {name} {decl}");
        conn.execute(&sql, [])?;
        info!("迁移: 为表 '{table}' 添加列 '{name}'");
    }
    Ok(())
}

/// 重命名列（如果存在，SQLite 3.25+）
pub fn rename_column(conn: &Connection, table: &str, old: &str, new: &str, _decl: &str) -> Result<()> {
    if column_exists(conn, table, old)? && !column_exists(conn, table, new)? {
        let sql = format!("ALTER TABLE {table} RENAME COLUMN {old} TO {new}");
        conn.execute(&sql, [])?;
        info!("迁移: 将表 '{table}' 的列 '{old}' 重命名为 '{new}'");
    }
    Ok(())
}

/// 删除索引（如果存在）
pub fn drop_index(conn: &Connection, name: &str) -> Result<()> {
    conn.execute(&format!("DROP INDEX IF EXISTS {name}"), [])?;
    Ok(())
}

/// 备份数据库文件（SQLite 在线备份）
pub fn backup_database(conn: &Connection, db_path: &Path) -> Result<()> {
    let parent = db_path
        .parent()
        .context("无法获取数据库所在目录")?;
    let backup_dir = parent.join("backup");
    std::fs::create_dir_all(&backup_dir)
        .context("无法创建 backup 目录")?;

    let stem = db_path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("无法解析数据库文件名")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let backup_path = backup_dir.join(format!("{stem}_{timestamp}.bak"));

    conn.backup(DatabaseName::Main, &backup_path, None)
        .with_context(|| format!("SQLite 备份失败 (目标: {:?})", backup_path))?;

    info!("迁移: 数据库已备份至 {:?}", backup_path);
    Ok(())
}
