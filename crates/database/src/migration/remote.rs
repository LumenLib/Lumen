use anyhow::Result;
use log::{debug, info};
use mysql_async::prelude::*;
use std::collections::HashSet;

/// 在远程 MySQL 数据库上执行所有待运行的迁移
///
/// 每个迁移版本匹配到对应版本的 `apply_remote()` 函数执行，
/// 无需远程改动的版本则跳过（仍会记录版本号到 `schema_version`）。
pub async fn run_remote_migrations(conn: &mut mysql_async::Conn) -> Result<()> {
    conn.query_drop(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version VARCHAR(32) PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at BIGINT NOT NULL
        )",
    )
    .await?;

    let rows: Vec<mysql_async::Row> = conn.exec("SELECT version FROM schema_version", ()).await?;
    let applied: HashSet<String> = rows.iter().filter_map(|r| r.get::<String, _>(0)).collect();

    let all = super::all_migrations();
    let pending: Vec<&super::Migration> = all
        .iter()
        .filter(|m| !applied.contains(m.version))
        .collect();

    if pending.is_empty() {
        debug!("远程迁移: 无需执行新迁移");
        return Ok(());
    }

    info!("远程迁移: 检测到 {} 个待执行迁移", pending.len());

    for m in &pending {
        debug!("远程迁移: 正在执行 {} - {}", m.version, m.description);
        dispatch_remote(conn, m.version).await?;
        conn.exec_drop(
            "INSERT INTO schema_version (version, description, applied_at) VALUES (?, ?, ?)",
            (m.version, m.description, chrono::Utc::now().timestamp()),
        )
        .await?;
        info!("远程迁移: {} - {} 已完成", m.version, m.description);
    }

    Ok(())
}

/// 按版本号分发到对应迁移的远程执行函数
///
/// 新版本在此添加 match arm：
/// ```ignore
/// "v020" => v020::apply_remote(conn).await?,
/// ```
async fn dispatch_remote(conn: &mut mysql_async::Conn, version: &str) -> Result<()> {
    match version {
        "v011" => {
            conn.exec_drop(
                "UPDATE annotations SET color = 'Magenta' WHERE color = 'Pink'",
                (),
            )
            .await?;
            conn.exec_drop(
                "UPDATE annotations SET color = 'Gray' WHERE color = 'Cyan'",
                (),
            )
            .await?;
        }
        _ => {}
    }
    Ok(())
}
