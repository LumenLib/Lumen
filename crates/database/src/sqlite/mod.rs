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
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
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
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
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
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
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
                    updated_at TEXT NOT NULL DEFAULT '',
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
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
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
                    updated_at TEXT NOT NULL DEFAULT '',
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
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT ''
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
                    updated_at TEXT NOT NULL DEFAULT '',
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
                    is_main BOOLEAN DEFAULT 0,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;

            // 9. 订阅 Feeds 表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS feeds (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    feed_type TEXT NOT NULL,
                    url TEXT,
                    last_updated_at TEXT,
                    update_interval INTEGER DEFAULT 24,
                    is_dirty BOOLEAN DEFAULT 1,
                    is_deleted BOOLEAN DEFAULT 0,
                    version INTEGER DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;

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
                    updated_at TEXT NOT NULL
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
                    updated_at TEXT NOT NULL DEFAULT '',
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

    /// 初始化默认数据（内置文件夹和订阅源），仅在首次运行时生效
    fn init_default_data(&self) -> Result<()> {
        debug!("初始化默认数据（内置文件夹和订阅源）");
        self.with_conn(|conn| {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

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

    pub fn clear_all_is_dirty(&self) -> Result<()> {
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
                conn.execute(&format!("UPDATE {table} SET is_dirty = 0"), [])?;
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
