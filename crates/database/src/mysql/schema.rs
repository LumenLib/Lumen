use super::MySqlManager;
use anyhow::Result;
use log::{error, info};
use mysql_async::prelude::*;

pub async fn ensure_remote_tables(conn: &mut mysql_async::Conn) -> Result<()> {
    let create_tables = [
        "CREATE TABLE IF NOT EXISTS literatures (id VARCHAR(64) PRIMARY KEY, title TEXT NOT NULL, year INT, month INT, day INT, type VARCHAR(32) NOT NULL, publication_id VARCHAR(64), volume VARCHAR(64), issue VARCHAR(64), pages VARCHAR(64), abstract_text MEDIUMTEXT, doi VARCHAR(256), arxiv_id VARCHAR(64), url TEXT, notes TEXT, keywords TEXT, rating INT DEFAULT 0, reading_status VARCHAR(32) DEFAULT 'Unread', is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS publications (id VARCHAR(64) PRIMARY KEY, name VARCHAR(255) NOT NULL, publication_type VARCHAR(32) NOT NULL, abbreviation VARCHAR(255), publisher VARCHAR(255), ccf_rank VARCHAR(32), jcr_rank VARCHAR(32), cas_rank VARCHAR(32), is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS authors (id VARCHAR(64) PRIMARY KEY, first_name VARCHAR(255) NOT NULL, last_name VARCHAR(255) NOT NULL, middle_name VARCHAR(255), is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_authors (literature_id VARCHAR(64) NOT NULL, author_id VARCHAR(64) NOT NULL, sort_order INT DEFAULT 0, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, PRIMARY KEY (literature_id, author_id), INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS folders (id VARCHAR(64) PRIMARY KEY, name VARCHAR(255) NOT NULL, folder_type VARCHAR(32) NOT NULL, parent_id VARCHAR(64), is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_folders (literature_id VARCHAR(64) NOT NULL, folder_id VARCHAR(64) NOT NULL, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, PRIMARY KEY (literature_id, folder_id), INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS tags (id VARCHAR(64) PRIMARY KEY, name VARCHAR(255) NOT NULL, color VARCHAR(32) DEFAULT '#808080', is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_tags (literature_id VARCHAR(64) NOT NULL, tag_id VARCHAR(64) NOT NULL, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, PRIMARY KEY (literature_id, tag_id), INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS attachments (id VARCHAR(64) PRIMARY KEY, literature_id VARCHAR(64) NOT NULL, file_path TEXT NOT NULL, file_name VARCHAR(255) NOT NULL, file_size BIGINT UNSIGNED NOT NULL, mime_type VARCHAR(128), etag VARCHAR(255), is_main BOOLEAN DEFAULT 0, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS feeds (id VARCHAR(64) PRIMARY KEY, name VARCHAR(255) NOT NULL, feed_type VARCHAR(32) NOT NULL, url TEXT, last_updated_at DATETIME, update_interval INT DEFAULT 24, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS feed_items (id VARCHAR(64) PRIMARY KEY, title TEXT NOT NULL, feed_id VARCHAR(64) NOT NULL, is_read BOOLEAN DEFAULT 0, is_added_to_library BOOLEAN DEFAULT 0, added_at DATETIME NOT NULL, authors TEXT, year INT, type VARCHAR(32), journal TEXT, publisher TEXT, abstract_text MEDIUMTEXT, doi VARCHAR(256), url TEXT, volume VARCHAR(64), issue VARCHAR(64), pages VARCHAR(64), published_at DATETIME, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_citations (source_id VARCHAR(64) NOT NULL, target_id VARCHAR(64) NOT NULL, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, PRIMARY KEY (source_id, target_id), INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS annotations (id VARCHAR(64) PRIMARY KEY, document_id VARCHAR(64) NOT NULL, page INT NOT NULL, kind VARCHAR(32) NOT NULL, color VARCHAR(32) NOT NULL, `range` TEXT, note TEXT, rect_x FLOAT, rect_y FLOAT, rect_w FLOAT, rect_h FLOAT, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at DATETIME NOT NULL, updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, INDEX (updated_at)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
    ];
    for sql in create_tables {
        conn.query_drop(sql).await?;
    }
    Ok(())
}

pub async fn clear_all_data(manager: &MySqlManager) -> Result<()> {
    let (use_remote, host, db_name) = {
        let c = manager.config.read().unwrap();
        (c.use_remote, c.host.clone(), c.database.clone())
    };
    if !use_remote {
        info!("MySQL: 远程同步未启用，跳过清空操作");
        return Ok(());
    }
    info!("MySQL: 开始彻底清空远程数据库: {host} (库名: {db_name})");
    let pool = manager.get_pool().await?;
    let mut conn = pool.get_conn().await?;

    let tables: Vec<String> = conn
        .exec(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = :db",
            params! { "db" => &db_name },
        )
        .await?;
    info!(
        "MySQL: [清理阶段] 发现当前数据库中共有 {} 个表",
        tables.len()
    );

    info!("MySQL: [清理阶段] 正在禁用外键约束检查...");
    conn.query_drop("SET FOREIGN_KEY_CHECKS = 0").await?;

    for table in &tables {
        info!("MySQL: [清理阶段] 正在处理表 '{table}'...");
        let drop_sql = format!("DROP TABLE IF EXISTS `{table}`");
        if let Err(e) = conn.query_drop(&drop_sql).await {
            error!("MySQL: [错误] 删除表 '{table}' 失败: {e}");
        } else {
            info!("MySQL: [成功] 表 '{table}' 已删除");
        }
    }

    info!("MySQL: [清理阶段] 正在重新启用外键约束检查...");
    conn.query_drop("SET FOREIGN_KEY_CHECKS = 1").await?;

    info!("MySQL: [清理阶段] 正在重新验证/初始化核心表结构...");
    ensure_remote_tables(&mut conn).await?;

    info!("MySQL: 远程数据库彻底清空并重置完成");
    Ok(())
}
