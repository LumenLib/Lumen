use log::{debug, error, info};
use rusqlite::{Connection, Result, Transaction, params};
use std::{fmt, path::Path, path::PathBuf, sync::Mutex};

pub mod annotation;
pub mod attachment;
pub mod author;

pub mod citation;
pub mod feed;
pub mod feed_item;
pub mod folder;
pub mod literature;
pub mod literature_notes;
pub mod publication;
pub mod tag;

/// 数据库管理器
pub struct Database {
    /// 使用 Mutex 确保 Connection 在多线程环境下是 Sync 的
    conn: Mutex<Connection>,
    /// 数据库文件路径（用于迁移备份）
    db_path: PathBuf,
}

impl fmt::Debug for Database {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Database").finish()
    }
}

impl Database {
    /// 创建并初始化数据库
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db_path = path.as_ref().to_path_buf();
        info!("正在打开本地数据库: {db_path:?}");
        let conn = Connection::open(&db_path)?;
        let db = Self {
            conn: Mutex::new(conn),
            db_path,
        };
        db.init_tables()?;
        db.run_local_migrations()?;
        db.init_default_data()?;
        Ok(db)
    }

    /// 内部辅助方法：获取连接锁并执行数据库操作
    pub(crate) fn with_conn<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R>,
    {
        let conn = self
            .conn
            .lock()
            .map_err(|_| rusqlite::Error::ExecuteReturnedResults)?;
        f(&conn)
    }

    /// 内部辅助方法：获取连接锁并在显式事务中执行数据库操作
    pub(crate) fn with_transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Transaction) -> Result<R>,
    {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| rusqlite::Error::ExecuteReturnedResults)?;
        let tx = conn.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }

    /// 初始化所有数据表
    fn init_tables(&self) -> Result<()> {
        debug!("正在初始化数据库表结构...");
        self.with_conn(|conn| {
            // 1. 文献主表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS literatures (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    year INTEGER,
                    month INTEGER,
                    day INTEGER,
                    type TEXT NOT NULL,
                    publication_id TEXT,
                    volume TEXT,
                    issue TEXT,
                    pages TEXT,
                    abstract_text TEXT,
                    doi TEXT,
                    arxiv_id TEXT,
                    url TEXT,
                    rating INTEGER DEFAULT 0,
                    reading_status TEXT DEFAULT 'Unread',
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 出版源表 (期刊/会议/书籍)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS publications (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    publication_type TEXT NOT NULL,
                    abbreviation TEXT,
                    publisher TEXT,
                    ccf_rank TEXT,
                    jcr_rank TEXT,
                    cas_rank TEXT,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 2. 作者表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS authors (
                    id TEXT PRIMARY KEY,
                    first_name TEXT NOT NULL,
                    last_name TEXT NOT NULL,
                    middle_name TEXT,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 3. 关联表：文献-作者 (多对多)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS literature_authors (
                    literature_id TEXT NOT NULL,
                    author_id TEXT NOT NULL,
                    sort_order INTEGER DEFAULT 0,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (literature_id, author_id)
                )",
                [],
            )?;

            // 4. 文件夹/分类表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS folders (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    folder_type TEXT NOT NULL,
                    parent_id TEXT,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 5. 关联表：文献-文件夹
            conn.execute(
                "CREATE TABLE IF NOT EXISTS literature_folders (
                    literature_id TEXT NOT NULL,
                    folder_id TEXT NOT NULL,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (literature_id, folder_id)
                )",
                [],
            )?;

            // 6. 标签表 (升级为独立实体)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS tags (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    color TEXT DEFAULT '#808080',
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 确保未删除的标签名称唯一 (应用层辅助，但数据库层也加上唯一索引以防万一)
            // 注意：这里使用部分索引 (Partial Index) 来允许软删除的同名标签存在
            conn.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_tags_name_active ON tags(name) WHERE is_deleted = 0",
                [],
            )?;

            // 7. 关联表：文献-标签
            conn.execute(
                "CREATE TABLE IF NOT EXISTS literature_tags (
                    literature_id TEXT NOT NULL,
                    tag_id TEXT NOT NULL,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (literature_id, tag_id)
                )",
                [],
            )?;

            // 8. 附件表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS attachments (
                    id TEXT PRIMARY KEY,
                    literature_id TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_name TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    mime_type TEXT,
                    etag TEXT,
                    hash TEXT,
                    is_main BOOLEAN DEFAULT 0,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 9. 订阅 Feeds 表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS feeds (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    title TEXT,
                    feed_type TEXT NOT NULL,
                    url TEXT,
                    last_updated_at TEXT,
                    update_interval INTEGER DEFAULT 24,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;
            // 兼容旧库：已存在的 feeds 表可能缺少 title 列
            let has_title: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('feeds') WHERE name='title'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .map(|c| c > 0)
                .unwrap_or(false);
            if !has_title {
                conn.execute("ALTER TABLE feeds ADD COLUMN title TEXT", [])?;
            }

            // 10. 订阅条目表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS feed_items (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    feed_id TEXT NOT NULL,
                    is_read BOOLEAN DEFAULT 0,
                    is_added_to_library BOOLEAN DEFAULT 0,
                    added_at TEXT NOT NULL,
                    authors TEXT,
                    year INTEGER,
                    type TEXT,
                    journal TEXT,
                    publisher TEXT,
                    abstract_text TEXT,
                    doi TEXT,
                    url TEXT,
                    volume TEXT,
                    issue TEXT,
                    pages TEXT,
                    published_at TEXT,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT 0
                )",
                [],
            )?;

            // 11. 引用关系表 (文献之间的引用关系)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS literature_citations (
                    source_id TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    updated_at INTEGER NOT NULL DEFAULT 0,
                    PRIMARY KEY (source_id, target_id)
                )",
                [],
            )?;

            // 12. 同步元数据表 (记录上次同步时间等)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS sync_meta (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                )",
                [],
            )?;

            // 13. PDF 注释表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS annotations (
                    id TEXT PRIMARY KEY,
                    document_id TEXT NOT NULL,
                    page INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    color TEXT NOT NULL,
                    range TEXT, -- JSON
                    note TEXT,
                    rect_x REAL,
                    rect_y REAL,
                    rect_w REAL,
                    rect_h REAL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    version INTEGER DEFAULT 1,
                    is_deleted INTEGER DEFAULT 0,
                    is_dirty INTEGER DEFAULT 0
                )",
                [],
            )?;

            // 为文档 ID 和页码创建索引
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_annotations_doc_page ON annotations(document_id, page)",
                [],
            )?;

            // 14. 文献笔记独立表（从 literatures.notes 拆分，1:N 多笔记）
            conn.execute(
                "CREATE TABLE IF NOT EXISTS literature_notes (
                    id              TEXT PRIMARY KEY,
                    literature_id   TEXT NOT NULL,
                    title           TEXT NOT NULL DEFAULT '',
                    content         TEXT NOT NULL DEFAULT '',
                    sort_order      INTEGER NOT NULL DEFAULT 0,
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL DEFAULT 0,
                    is_deleted      INTEGER DEFAULT 0,
                    is_dirty        INTEGER DEFAULT 0,
                    version         INTEGER DEFAULT 1
                )",
                [],
            )?;

            // 执行数据库迁移
            crate::migration::run_migrations(
                conn,
                &self.db_path,
                &crate::migration::all_migrations(),
            )
            .map_err(|e| {
                error!("数据库迁移失败: {e}");
                rusqlite::Error::ExecuteReturnedResults
            })?;

            Ok(())
        })
    }

    /// 把历史库中 `created_at`/`updated_at` 文本列转换为 INTEGER（Unix 秒）。
    ///
    /// 关键修正（上一版踩的坑）：
    /// 1. 以“真实列类型”判断是否仍需迁移，而不是只看 `_migrations` 版本号——
    ///    上一版曾错误地标记 v1 却没改成功列类型，纯靠版本号会永远跳过。
    /// 2. SQLite 不支持 `ALTER TABLE ... ALTER COLUMN` 改存储类，必须用
    ///    “建新表(INTEGER) -> 拷贝数据(CAST) -> 改名” 重建表，才能同时改列声明与存储类。
    /// 3. 迁移失败必须可见（不吞错、不报假成功）。
    fn run_local_migrations(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT '')",
                [],
            )?;

            if !Self::local_ts_columns_need_migration(conn)? {
                debug!("本地时间戳列已为 INTEGER，跳过迁移");
                return Ok(());
            }

            info!("本地数据库: 开始迁移时间戳列 TEXT -> INTEGER");
            conn.execute("PRAGMA foreign_keys = OFF", [])?;

            for &table in Self::TS_MIGRATION_TABLES {
                if Self::table_has_text_ts(conn, table)? {
                    Self::rebuild_table_timestamps(conn, table).map_err(|e| {
                        error!("本地迁移失败(表 {table}): {e}");
                        e
                    })?;
                }
            }

            conn.execute("PRAGMA foreign_keys = ON", [])?;

            if Self::local_ts_columns_need_migration(conn)? {
                error!("本地迁移校验失败：时间戳列仍非 INTEGER，迁移未完成");
                return Err(rusqlite::Error::ExecuteReturnedResults);
            }

            conn.execute(
                "INSERT OR REPLACE INTO _migrations (version, applied_at) VALUES (2, datetime('now'))",
                [],
            )?;
            info!("本地数据库: 时间戳列已迁移至 INTEGER (完成)");
            Ok(())
        })
    }

    /// 参与时间戳迁移的表（拥有 created_at / updated_at 列）。
    const TS_MIGRATION_TABLES: &'static [&'static str] = &[
        "literatures",
        "publications",
        "authors",
        "literature_authors",
        "folders",
        "literature_folders",
        "tags",
        "literature_tags",
        "attachments",
        "feeds",
        "feed_items",
        "literature_citations",
        "annotations",
        "literature_notes",
    ];

    /// 是否还有任何表的 created_at / updated_at 仍是 TEXT（需要迁移）。
    fn local_ts_columns_need_migration(conn: &Connection) -> Result<bool> {
        for &table in Self::TS_MIGRATION_TABLES {
            if Self::table_has_text_ts(conn, table)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 该表是否存在 created_at / updated_at 中任一为 TEXT 的列（表不存在则 false）。
    fn table_has_text_ts(conn: &Connection, table: &str) -> Result<bool> {
        if !Self::table_exists(conn, table)? {
            return Ok(false);
        }
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            let ctype: String = row.get(2)?;
            Ok((name, ctype))
        })?;
        for r in rows {
            let (name, ctype) = r?;
            if (name == "created_at" || name == "updated_at") && ctype.eq_ignore_ascii_case("TEXT")
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// 表是否存在（table / view 都算）。
    fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
        let c: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?",
            [name],
            |r| r.get(0),
        )?;
        Ok(c > 0)
    }

    /// 用“建新表(INTEGER) -> 拷贝数据(CAST) -> 改名”真正改变存储类。
    /// SQLite 不支持 ALTER COLUMN 改存储类，必须重建表；同时保留二级索引。
    pub(crate) fn rebuild_table_timestamps(conn: &Connection, table: &str) -> Result<()> {
        let tmp = format!("{table}__mig_tmp");
        let has_tbl = Self::table_exists(conn, table)?;
        let has_tmp = Self::table_exists(conn, &tmp)?;
        if has_tmp {
            if has_tbl {
                // 新表已建好，仅残留临时表，清理即可
                conn.execute(&format!("DROP TABLE IF EXISTS {tmp}"), [])?;
                return Ok(());
            }
            // 仅残留临时表：旧表在“改名后建新表”前失败，还原回旧表，下次启动重试
            conn.execute(&format!("ALTER TABLE {tmp} RENAME TO {table}"), [])?;
            return Ok(());
        }

        // 捕获二级索引 DDL（自动索引 sql 为 NULL，会被排除），稍后重建
        let mut idx_defs: Vec<(String, String)> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT name, sql FROM sqlite_master WHERE type='index' AND tbl_name=? AND sql IS NOT NULL",
            )?;
            let rows = stmt.query_map([table], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for r in rows {
                idx_defs.push(r?);
            }
        }

        // 读取原表列信息（名称 + 是否时间戳文本列）
        let mut cols: Vec<(String, bool)> = Vec::new();
        {
            let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                let ctype: String = row.get(2)?;
                let is_ts = (name == "created_at" || name == "updated_at")
                    && ctype.eq_ignore_ascii_case("TEXT");
                Ok((name, is_ts))
            })?;
            for r in rows {
                cols.push(r?);
            }
        }
        if cols.is_empty() {
            return Ok(()); // 表不存在，跳过
        }

        // 取原建表语句并替换时间戳列类型为 INTEGER
        let create_sql: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
            [table],
            |row| row.get::<_, String>(0),
        )?;
        let new_create = create_sql
            .replace("created_at TEXT", "created_at INTEGER")
            .replace("updated_at TEXT", "updated_at INTEGER");

        // 先删二级索引（避免改名后索引名冲突），再重建表
        for (name, _sql) in &idx_defs {
            conn.execute(&format!("DROP INDEX IF EXISTS {name}"), [])?;
        }
        conn.execute(&format!("ALTER TABLE {table} RENAME TO {tmp}"), [])?;
        conn.execute(&new_create, [])?;

        // 构造 INSERT ... SELECT，时间戳列做转换：
        // 数字串 -> 直接 CAST；datetime 串 -> strftime('%s')；NULL/空 -> 0
        let col_names: Vec<String> = cols.iter().map(|(n, _)| n.clone()).collect();
        let col_list = col_names.join(", ");
        let select_exprs: Vec<String> = cols
            .iter()
            .map(|(n, is_ts)| {
                if *is_ts {
                    format!(
                        "CASE WHEN {n} IS NULL OR {n} = '' THEN 0 \
                         WHEN {n} GLOB '[0-9]*' AND {n} NOT GLOB '*[^0-9]*' THEN CAST({n} AS INTEGER) \
                         ELSE CAST(strftime('%s', {n}) AS INTEGER) END"
                    )
                } else {
                    n.clone()
                }
            })
            .collect();
        let select_list = select_exprs.join(", ");
        conn.execute(
            &format!("INSERT INTO {table} ({col_list}) SELECT {select_list} FROM {tmp}"),
            [],
        )?;

        // 重建二级索引
        for (_name, sql) in &idx_defs {
            conn.execute(sql, [])?;
        }

        conn.execute(&format!("DROP TABLE {tmp}"), [])?;
        info!("本地迁移: 表 {table} 时间戳列已转为 INTEGER");
        Ok(())
    }

    /// 初始化默认数据（内置文件夹和订阅源），仅在首次运行时生效
    fn init_default_data(&self) -> Result<()> {
        debug!("初始化默认数据（内置文件夹和订阅源）");
        self.with_conn(|conn| {
            let now = chrono::Local::now().timestamp();

            // 内置文件夹（INSERT OR IGNORE 确保仅首次创建）
            conn.execute(
                "INSERT OR IGNORE INTO folders (id, name, folder_type, parent_id, is_dirty, is_deleted, version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 1, 0, 1, ?4, ?4)",
                params!["all", "All Literature", "all", now],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO folders (id, name, folder_type, parent_id, is_dirty, is_deleted, version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 1, 0, 1, ?4, ?4)",
                params!["uncategorized", "Uncategorized", "uncategorized", now],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO folders (id, name, folder_type, parent_id, is_dirty, is_deleted, version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 1, 0, 1, ?4, ?4)",
                params!["trash", "Trash", "trash", now],
            )?;

            // 内置订阅源
            conn.execute(
                "INSERT OR IGNORE INTO feeds (id, name, feed_type, url, update_interval, is_dirty, is_deleted, version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 24, 1, 0, 1, ?4, ?4)",
                params!["all_subs", "All Subscriptions", "rss", now],
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO feeds (id, name, feed_type, url, update_interval, is_dirty, is_deleted, version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, NULL, 24, 1, 0, 1, ?4, ?4)",
                params!["unread", "Unread", "rss", now],
            )?;

            debug!("默认数据初始化完成");
            Ok(())
        })
    }

    /// 获取所有表名
    fn get_table_names(&self) -> Vec<&'static str> {
        vec![
            "literatures",
            "publications",
            "authors",
            "literature_authors",
            "folders",
            "literature_folders",
            "tags",
            "literature_tags",
            "attachments",
            "feeds",
            "feed_items",
            "sync_meta",
            "literature_citations",
            "annotations",
            "literature_notes",
        ]
    }

    pub fn clear_all(&self) -> Result<()> {
        info!("数据库: 正在清空本地数据库所有表数据...");
        let tables = self.get_table_names();
        self.with_conn(|conn| {
            for table in &tables {
                match conn.execute(&format!("DELETE FROM {table}"), []) {
                    Ok(count) => info!("数据库: 表 '{table}' 已清空 (物理删除 {count} 条记录)"),
                    Err(e) => error!("数据库: 清空表 '{table}' 失败: {e}"),
                }
            }
            Ok(())
        })
    }

    fn drop_all_tables(&self) -> Result<()> {
        info!("警告: 正在删除数据库所有表!");
        let tables = self.get_table_names();
        self.with_conn(|conn| {
            for table in tables {
                debug!("正在删除表: {table}");
                conn.execute(&format!("DROP TABLE IF EXISTS {table}"), [])?;
            }
            Ok(())
        })
    }

    pub fn rebuild_schema(&self) -> Result<()> {
        info!("正在重建数据库结构...");
        self.drop_all_tables()?;

        self.init_tables()?;

        Ok(())
    }

    // --- 同步元数据管理 ---

    pub fn get_sync_meta(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT value FROM sync_meta WHERE key = ?1")?;

            let mut rows = stmt.query([key])?;

            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
    }

    pub fn set_sync_meta(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?1, ?2)",
                [key, value],
            )?;

            Ok(())
        })
    }

    pub fn get_last_sync_time(&self, table: &str) -> Result<Option<String>> {
        self.get_sync_meta(&format!("last_sync_{table}"))
    }

    pub fn set_last_sync_time(&self, table: &str, time: &str) -> Result<()> {
        self.set_sync_meta(&format!("last_sync_{table}"), time)
    }

    pub fn mark_all_dirty_for_sync(&self) -> Result<()> {
        self.with_conn(|conn| {
            let tables = [
                "literatures",
                "authors",
                "folders",
                "tags",
                "literature_authors",
                "literature_folders",
                "literature_tags",
                "attachments",
                "feeds",
                "feed_items",
                "literature_citations",
                "annotations",
                "literature_notes",
            ];
            for table in tables {
                conn.execute(&format!("UPDATE {table} SET is_dirty = 1"), [])?;
            }
            Ok(())
        })
    }

    pub fn clear_sync_timestamps(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM sync_meta WHERE key LIKE 'last_sync_%'", [])?;
            Ok(())
        })
    }

    pub fn clear_attachment_etags(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("UPDATE attachments SET etag = NULL", [])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// 验证“建新表 -> 拷贝(CAST) -> 改名”能真正把 TEXT 时间戳列转为 INTEGER，
    /// 且 epoch 文本与 datetime 文本都被正确转成整数（不截断、不损坏）。
    #[test]
    fn text_timestamp_columns_convert_to_integer() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE literatures (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        // 模拟“上一版错误迁移”留下的 epoch 文本，以及一份原始 datetime 文本
        conn.execute(
            "INSERT INTO literatures VALUES ('a', '1780886744', '2026-07-21 00:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO literatures VALUES ('b', '2026-07-21 00:00:00', '1784571065')",
            [],
        )
        .unwrap();

        // 迁移前：列仍是 TEXT
        assert!(Database::table_has_text_ts(&conn, "literatures").unwrap());

        // 执行重建式迁移
        Database::rebuild_table_timestamps(&conn, "literatures").unwrap();

        // 迁移后：列类型必须为 INTEGER
        let types: Vec<(String, String)> = {
            let mut stmt = conn.prepare("PRAGMA table_info(literatures)").unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
                .unwrap();
            rows.map(|x| x.unwrap()).collect()
        };
        for (name, t) in &types {
            if name == "created_at" || name == "updated_at" {
                assert_eq!(t.to_uppercase(), "INTEGER", "列 {name} 应为 INTEGER");
            }
        }

        // 值应为整数且转换正确
        let expected_dated: i64 = conn
            .query_row(
                "SELECT CAST(strftime('%s','2026-07-21 00:00:00') AS INTEGER)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let vals: Vec<(i64, i64)> = {
            let mut stmt = conn
                .prepare("SELECT created_at, updated_at FROM literatures ORDER BY id")
                .unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
                .unwrap();
            rows.map(|x| x.unwrap()).collect()
        };
        assert_eq!(vals.len(), 2);
        assert_eq!(vals[0].0, 1780886744); // epoch 文本 -> 整数
        assert_eq!(vals[0].1, expected_dated); // datetime 文本 -> strftime 整数
        assert_eq!(vals[1].0, expected_dated);
        assert_eq!(vals[1].1, 1784571065);

        // 数据完整性：行数不变
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM literatures", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 2);
    }
}
