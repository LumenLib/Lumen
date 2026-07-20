use super::Database;
use log::{debug, info};
use models::{Publication, PublicationType};
use rusqlite::{OptionalExtension, Result, params};

impl Database {
    pub fn get_dirty_publications(&self) -> Result<Vec<Publication>> {
        debug!("数据库: 正在获取待同步出版源记录");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, name, publication_type, abbreviation, publisher, ccf_rank, jcr_rank, cas_rank, is_dirty, is_deleted, version, created_at, updated_at FROM publications WHERE is_dirty = 1")?;
            let iter = stmt.query_map([], |row| {
                 let pt_str: String = row.get(2)?;
                 let pt = match pt_str.as_str() {
                     "Journal" => PublicationType::Journal,
                     "Conference" => PublicationType::Conference,
                     "Book" => PublicationType::Book,
                     _ => PublicationType::Journal,
                 };
                 Ok(Publication {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    publication_type: pt,
                    abbreviation: row.get(3)?,
                    publisher: row.get(4)?,
                    ccf_rank: row.get(5)?,
                    jcr_rank: row.get(6)?,
                    cas_rank: row.get(7)?,
                    is_dirty: row.get(8)?,
                    is_deleted: row.get(9)?,
                    version: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?;
            let mut pubs = Vec::new();
            for p in iter { pubs.push(p?); }
            Ok(pubs)
        })
    }

    pub fn mark_publication_synced(&self, id: &str) -> Result<()> {
        debug!("数据库: 标记出版源为已同步 (ID: {id})");
        self.with_conn(|conn| {
            conn.execute("UPDATE publications SET is_dirty = 0 WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    pub fn merge_remote_publication(&self, remote: Publication) -> Result<()> {
        info!(
            "数据库: 正在合并远程出版源信息 (ID: {}, version: {})",
            remote.id, remote.version
        );
        self.with_conn(|conn| {
            let local_info: Option<(i32, bool)> = conn
                .query_row(
                    "SELECT version, is_dirty FROM publications WHERE id = ?1",
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
                    self._insert_publication_internal(conn, &remote)?;
                } else if remote.version == local_version && !is_dirty {
                    debug!("数据库: 版本一致且本地未修改，更新时间戳并标记同步");
                    conn.execute(
                        "UPDATE publications SET updated_at = ?1, is_dirty = 0 WHERE id = ?2",
                        params![remote.updated_at, remote.id],
                    )?;
                } else {
                    debug!("数据库: 本地版本较新或有未同步修改，忽略远程更新");
                }
            } else {
                debug!("数据库: 本地未找到该出版源，执行插入");
                self._insert_publication_internal(conn, &remote)?;
            }
            Ok(())
        })
    }

    fn _insert_publication_internal(
        &self,
        conn: &rusqlite::Connection,
        pub_data: &Publication,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO publications (id, name, publication_type, abbreviation, publisher, ccf_rank, jcr_rank, cas_rank, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10, ?11, ?12)",
            params![
                pub_data.id,
                pub_data.name,
                pub_data.publication_type.to_string(),
                pub_data.abbreviation,
                pub_data.publisher,
                pub_data.ccf_rank,
                pub_data.jcr_rank,
                pub_data.cas_rank,
                pub_data.is_deleted,
                pub_data.version,
                pub_data.created_at,
                pub_data.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn purge_synced_publications(&self) -> Result<usize> {
        info!("数据库: 正在清理已同步的删除出版源记录");
        self.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM publications WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            info!("数据库: 已清理 {count} 个已同步删除的出版源");
            Ok(count)
        })
    }
}
