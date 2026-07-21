use super::Database;
use log::{debug, info};
use models::Author;
use rusqlite::{Connection, OptionalExtension, Result, params};

impl Database {
    /// 在事务内根据姓名查找作者 ID
    pub fn find_author_id_by_name(
        conn: &Connection,
        first: &str,
        last: &str,
        middle: Option<&str>,
    ) -> Result<Option<String>> {
        let middle = middle.unwrap_or("");
        conn.query_row(
            "SELECT id FROM authors WHERE first_name = ?1 AND last_name = ?2 AND COALESCE(middle_name, '') = ?3 AND is_deleted = 0 LIMIT 1",
            params![first, last, middle],
            |row| row.get(0),
        )
        .optional()
    }

    /// 在事务内插入或替换作者行
    pub fn upsert_author(conn: &Connection, author: &Author) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO authors (id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![author.id, author.first_name, author.last_name, author.middle_name, author.is_dirty, author.is_deleted, author.version, author.created_at, author.updated_at],
        )?;
        Ok(())
    }

    // --- 同步支持方法 ---

    pub fn get_dirty_authors(&self) -> Result<Vec<Author>> {
        debug!("数据库: 正在获取待同步作者记录");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at FROM authors WHERE is_dirty = 1")?;
            let iter = stmt.query_map([], |row| Ok(Author { id: row.get(0)?, first_name: row.get(1)?, last_name: row.get(2)?, middle_name: row.get(3)?, is_dirty: row.get(4)?, is_deleted: row.get(5)?, version: row.get(6)?, created_at: row.get(7)?, updated_at: row.get(8)? }))?;
            let mut authors = Vec::new();
            for a in iter { authors.push(a?); }
            debug!("数据库: 获取到 {} 个待同步作者", authors.len());
            Ok(authors)
        })
    }

    pub fn mark_author_synced(&self, id: &str) -> Result<()> {
        debug!("数据库: 标记作者为已同步 (ID: {id})");
        self.with_conn(|conn| {
            conn.execute("UPDATE authors SET is_dirty = 0 WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn merge_remote_author(&self, remote: Author) -> Result<()> {
        info!(
            "数据库: 正在合并远程作者信息 (ID: {}, version: {})",
            remote.id, remote.version
        );
        self.with_conn(|conn| {
            let local_info: Option<(i32, bool)> = conn
                .query_row(
                    "SELECT version, is_dirty FROM authors WHERE id = ?1",
                    [&remote.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((local_version, is_dirty)) = local_info {
                if remote.version > local_version {
                    debug!(
                        "数据库: 远程版本较新 ({} > {})，执行覆盖更新",
                        remote.version, local_version
                    );
                    Self::insert_author_internal(conn, &remote)?;
                } else if remote.version == local_version && !is_dirty {
                    debug!("数据库: 版本一致且本地未修改，更新时间戳并标记同步");
                    conn.execute(
                        "UPDATE authors SET updated_at = ?1, is_dirty = 0 WHERE id = ?2",
                        params![remote.updated_at, remote.id],
                    )?;
                } else {
                    debug!("数据库: 本地版本较新或有未同步修改，忽略远程更新");
                }
            } else {
                debug!("数据库: 本地未找到该作者，执行插入");
                Self::insert_author_internal(conn, &remote)?;
            }
            Ok(())
        })
    }

    fn insert_author_internal(conn: &Connection, author: &Author) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO authors (id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![author.id, author.first_name, author.last_name, author.middle_name, 0, author.is_deleted, author.version, author.created_at, author.updated_at],
        )?;
        Ok(())
    }

    pub fn purge_synced_authors(&self) -> Result<usize> {
        info!("数据库: 正在清理已同步的删除作者记录");
        self.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM authors WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            info!("数据库: 已清理 {count} 个已同步删除的作者");
            Ok(count)
        })
    }
}
