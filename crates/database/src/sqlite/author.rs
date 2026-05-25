use super::Database;
use chrono::Local;
use log::{debug, info, warn};
use models::Author;
use rusqlite::{OptionalExtension, Result, params};

impl Database {
    pub fn insert_author(&self, author: &Author) -> Result<()> {
        info!(
            "数据库: 准备写入作者 '{} {}' (ID: {})",
            author.first_name, author.last_name, author.id
        );
        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO authors (id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![author.id, author.first_name, author.last_name, author.middle_name, author.is_dirty, author.is_deleted, author.version, author.created_at, author.updated_at],
            )?;
            Ok(())
        })
    }

    pub fn get_all_authors(&self) -> Result<Vec<Author>> {
        debug!("数据库: 正在获取所有作者列表");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at FROM authors WHERE is_deleted = 0 ORDER BY last_name, first_name")?;
            let author_iter = stmt.query_map([], |row| {
                Ok(Author {
                    id: row.get(0)?,
                    first_name: row.get(1)?,
                    last_name: row.get(2)?,
                    middle_name: row.get(3)?,
                    is_dirty: row.get(4)?,
                    is_deleted: row.get(5)?,
                    version: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?;

            let mut authors = Vec::new();
            for author in author_iter {
                authors.push(author?);
            }
            debug!("数据库: 成功获取 {} 个作者", authors.len());
            Ok(authors)
        })
    }

    pub fn get_authors_for_literature(&self, literature_id: &str) -> Result<Vec<Author>> {
        debug!("数据库: 正在获取文献 (ID: {literature_id}) 的作者");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT a.id, a.first_name, a.last_name, a.middle_name, a.is_dirty, a.is_deleted, a.version, a.created_at, a.updated_at
                 FROM authors a
                 JOIN literature_authors la ON a.id = la.author_id
                 WHERE la.literature_id = ?1 AND a.is_deleted = 0
                 ORDER BY la.sort_order",
            )?;
            let author_iter = stmt.query_map([literature_id], |row| {
                Ok(Author {
                    id: row.get(0)?,
                    first_name: row.get(1)?,
                    last_name: row.get(2)?,
                    middle_name: row.get(3)?,
                    is_dirty: row.get(4)?,
                    is_deleted: row.get(5)?,
                    version: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?;

            let mut authors = Vec::new();
            for author in author_iter {
                authors.push(author?);
            }
            debug!("数据库: 文献 {} 共有 {} 个作者", literature_id, authors.len());
            Ok(authors)
        })
    }

    pub fn update_author(&self, author: &Author) -> Result<()> {
        info!(
            "数据库: 正在更新作者信息 '{} {}' (ID: {})",
            author.first_name, author.last_name, author.id
        );
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE authors SET first_name = ?1, last_name = ?2, middle_name = ?3, is_dirty = 1, version = version + 1, updated_at = ?4 WHERE id = ?5",
                params![author.first_name, author.last_name, author.middle_name, Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), author.id],
            )?;
            if rows == 0 {
                warn!("数据库: 更新作者失败，未找到 ID 为 {} 的记录", author.id);
            }
            Ok(())
        })
    }

    pub fn delete_author(&self, id: &str) -> Result<()> {
        info!("数据库: 准备删除作者 (ID: {id})");
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE authors SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE id = ?2",
                params![Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), id],
            )?;
            if rows > 0 {
                debug!("数据库: 作者 (ID: {id}) 已标记为删除");
            } else {
                warn!("数据库: 删除作者失败，未找到 ID 为 {id} 的记录");
            }
            Ok(())
        })
    }

    /// 根据姓名查找作者（用于重名检测）
    pub fn find_authors_by_name(
        &self,
        first: &str,
        last: &str,
        middle: Option<&str>,
    ) -> Result<Vec<Author>> {
        debug!("数据库: 正在查找作者姓名 '{first} {last}' (middle: {middle:?})");
        self.with_conn(|conn| {
            let mut authors = Vec::new();
            if let Some(m) = middle {
                let mut stmt = conn.prepare("SELECT id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at FROM authors WHERE first_name = ?1 AND last_name = ?2 AND middle_name = ?3 AND is_deleted = 0")?;
                let iter = stmt.query_map(params![first, last, m], |row| {
                    Ok(Author {
                        id: row.get(0)?,
                        first_name: row.get(1)?,
                        last_name: row.get(2)?,
                        middle_name: row.get(3)?,
                        is_dirty: row.get(4)?,
                        is_deleted: row.get(5)?,
                        version: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                for a in iter { authors.push(a?); }
            } else {
                let mut stmt = conn.prepare("SELECT id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at FROM authors WHERE first_name = ?1 AND last_name = ?2 AND middle_name IS NULL AND is_deleted = 0")?;
                let iter = stmt.query_map(params![first, last], |row| {
                    Ok(Author {
                        id: row.get(0)?,
                        first_name: row.get(1)?,
                        last_name: row.get(2)?,
                        middle_name: row.get(3)?,
                        is_dirty: row.get(4)?,
                        is_deleted: row.get(5)?,
                        version: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?;
                for a in iter { authors.push(a?); }
            }
            debug!("数据库: 查找到 {} 个同名作者", authors.len());
            Ok(authors)
        })
    }

    /// 合并作者
    /// 将 `source_id` 的所有关联迁移到 `target_id，然后删除` `source_id`
    pub fn merge_authors(&self, source_id: &str, target_id: &str) -> Result<()> {
        if source_id == target_id {
            return Ok(());
        }
        info!("数据库: 正在合并作者 ({source_id} -> {target_id})");
        self.with_conn(|conn| {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            // 找出 source_id 参与的所有文献
            let mut stmt = conn.prepare("SELECT literature_id FROM literature_authors WHERE author_id = ?1 AND is_deleted = 0")?;
            let lit_ids: Vec<String> = stmt.query_map([source_id], |row| row.get(0))?.collect::<Result<Vec<_>>>()?;

            info!("数据库: 迁移作者关联，涉及 {} 篇文献", lit_ids.len());
            for lit_id in lit_ids {
                // 检查 target_id 是否已经关联了这篇文献
                let already_exists: bool = conn.query_row(
                    "SELECT 1 FROM literature_authors WHERE literature_id = ?1 AND author_id = ?2 AND is_deleted = 0",
                    params![lit_id, target_id], |_| Ok(true)
                ).optional()?.unwrap_or(false);

                if already_exists {
                    // 如果已经存在，软删除 source 的关联
                    debug!("数据库: 文献 {lit_id} 已关联目标作者，软删除源作者关联");
                    conn.execute(
                        "UPDATE literature_authors SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE literature_id = ?2 AND author_id = ?3",
                        params![now, lit_id, source_id]
                    )?;
                } else {
                    // 如果不存在，将 source 的关联修改为 target
                    debug!("数据库: 将文献 {lit_id} 的作者从 {source_id} 修改为 {target_id}");
                    conn.execute(
                        "UPDATE literature_authors SET author_id = ?1, is_dirty = 1, version = version + 1, updated_at = ?2 WHERE literature_id = ?3 AND author_id = ?4",
                        params![target_id, now, lit_id, source_id]
                    )?;
                }
            }
            // 最后软删除 source 作者本人
            info!("数据库: 软删除被合并的作者 (ID: {source_id})");
            conn.execute("UPDATE authors SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE id = ?2", params![now, source_id])?;
            Ok(())
        })
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
                    self._insert_author_internal(conn, &remote)?;
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
                self._insert_author_internal(conn, &remote)?;
            }
            Ok(())
        })
    }

    fn _insert_author_internal(&self, conn: &rusqlite::Connection, author: &Author) -> Result<()> {
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
