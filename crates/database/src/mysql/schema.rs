use super::MySqlManager;
use anyhow::Result;
use log::{error, info};
use mysql_async::prelude::*;

pub async fn ensure_remote_tables(conn: &mut mysql_async::Conn) -> Result<()> {
    let create_tables = [
        "CREATE TABLE IF NOT EXISTS literatures (id VARCHAR(64) PRIMARY KEY, title TEXT NOT NULL, year INT, month INT, day INT, type TEXT NOT NULL, publication_id VARCHAR(64), volume TEXT, issue TEXT, pages TEXT, abstract_text MEDIUMTEXT, doi TEXT, arxiv_id TEXT, url TEXT, notes TEXT, keywords TEXT, rating INT DEFAULT 0, reading_status TEXT DEFAULT 'Unread', is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS publications (id VARCHAR(64) PRIMARY KEY, name TEXT NOT NULL, publication_type TEXT NOT NULL, abbreviation TEXT, publisher TEXT, ccf_rank TEXT, jcr_rank TEXT, cas_rank TEXT, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS authors (id VARCHAR(64) PRIMARY KEY, first_name TEXT NOT NULL, last_name TEXT NOT NULL, middle_name TEXT, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_authors (literature_id VARCHAR(64) NOT NULL, author_id VARCHAR(64) NOT NULL, sort_order INT DEFAULT 0, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at TEXT NOT NULL DEFAULT '', PRIMARY KEY (literature_id, author_id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS folders (id VARCHAR(64) PRIMARY KEY, name TEXT NOT NULL, folder_type TEXT NOT NULL, parent_id VARCHAR(64), is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_folders (literature_id VARCHAR(64) NOT NULL, folder_id VARCHAR(64) NOT NULL, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at TEXT NOT NULL DEFAULT '', PRIMARY KEY (literature_id, folder_id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS tags (id VARCHAR(64) PRIMARY KEY, name TEXT NOT NULL, color TEXT DEFAULT '#808080', is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_tags (literature_id VARCHAR(64) NOT NULL, tag_id VARCHAR(64) NOT NULL, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at TEXT NOT NULL DEFAULT '', PRIMARY KEY (literature_id, tag_id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS attachments (id VARCHAR(64) PRIMARY KEY, literature_id VARCHAR(64) NOT NULL, file_path TEXT NOT NULL, file_name TEXT NOT NULL, file_size BIGINT UNSIGNED NOT NULL, mime_type TEXT, etag TEXT, is_main BOOLEAN DEFAULT 0, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS feeds (id VARCHAR(64) PRIMARY KEY, name TEXT NOT NULL, feed_type TEXT NOT NULL, url TEXT, last_updated_at TEXT, update_interval INT DEFAULT 24, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS feed_items (id VARCHAR(64) PRIMARY KEY, title TEXT NOT NULL, feed_id VARCHAR(64) NOT NULL, is_read BOOLEAN DEFAULT 0, is_added_to_library BOOLEAN DEFAULT 0, added_at TEXT NOT NULL, authors TEXT, year INT, type TEXT, journal TEXT, publisher TEXT, abstract_text MEDIUMTEXT, doi TEXT, url TEXT, volume TEXT, issue TEXT, pages TEXT, published_at TEXT, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at TEXT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_citations (source_id VARCHAR(64) NOT NULL, target_id VARCHAR(64) NOT NULL, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, updated_at TEXT NOT NULL DEFAULT '', PRIMARY KEY (source_id, target_id)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS annotations (id VARCHAR(64) PRIMARY KEY, document_id TEXT NOT NULL, page INT NOT NULL, kind TEXT NOT NULL, color TEXT NOT NULL, `range` TEXT, note TEXT, rect_x DOUBLE, rect_y DOUBLE, rect_w DOUBLE, rect_h DOUBLE, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1, created_at BIGINT NOT NULL, updated_at BIGINT NOT NULL) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
        "CREATE TABLE IF NOT EXISTS literature_notes (id VARCHAR(64) PRIMARY KEY, literature_id VARCHAR(64) NOT NULL, title TEXT NOT NULL DEFAULT '', content TEXT NOT NULL DEFAULT '', sort_order INT NOT NULL DEFAULT 0, created_at BIGINT NOT NULL, updated_at BIGINT NOT NULL DEFAULT 0, is_deleted BOOLEAN DEFAULT 0, version INT DEFAULT 1) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;",
    ];
    for sql in create_tables {
        conn.query_drop(sql).await?;
    }

    // 索引（对等 SQLite） — 可能已存在，忽略重复创建错误
    let indexes = [
        "CREATE INDEX idx_annotations_doc_page ON annotations(document_id(64), page)",
        "CREATE UNIQUE INDEX idx_tags_name_active ON tags(name) WHERE is_deleted = 0",
    ];
    for sql in indexes {
        if let Err(e) = conn.query_drop(sql).await {
            info!("MySQL: 索引创建跳过 (可能已存在): {e}");
        }
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
