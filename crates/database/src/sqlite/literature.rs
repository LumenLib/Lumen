use super::Database;
use crate::constructors::*;
use anyhow::anyhow;
use chrono::Local;
use log::{debug, error, info, warn};
use models::{Attachment, Author, Literature, LiteratureType, Publication, PublicationType, Tag};
use rusqlite::{Connection, OptionalExtension, Result, Row, params};
use uuid::Uuid;

type AuthorRelationTuple = (String, String, Option<i32>, bool, i32);
type CommonRelationTuple = (String, String, bool, i32);

struct RelationUpsertData {
    lit_id: String,
    target_id: String,
    sort_order: Option<i32>,
    is_deleted: bool,
    version: i32,
}

impl Database {
    /// 插入或全量更新文献及其关联关系
    pub fn insert_literature(&self, lit: &Literature) -> Result<()> {
        info!(
            "数据库: 准备写入文献, ID: {}, Title: '{}'",
            lit.id, lit.title
        );
        debug!(
            "数据库: 写入文献详情: year={:?}, type={:?}, doi={:?}",
            lit.year, lit.literature_type, lit.doi
        );
        self.with_transaction(|tx| {
            // 首先处理出版源，获取 publication_id
            let publication_id = Self::_set_publication(tx, lit.publication.as_ref())?;

            tx.execute(
                "INSERT OR REPLACE INTO literatures (
                    id, title, year, month, day, type, publication_id, volume, issue, pages,
                    abstract_text, doi, arxiv_id, isbn, url, notes, rating, reading_status, is_dirty, is_deleted, version,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)",
                params![
                    lit.id, lit.title, lit.year, lit.month, lit.day,
                    serde_json::to_string(&lit.literature_type).unwrap_or_else(|_| "\"article\"".to_string()).trim_matches('"'),
                    publication_id, lit.volume, lit.issue, lit.pages,
                    lit.abstract_text, lit.doi, lit.arxiv_id, lit.isbn, lit.url, lit.notes,
                    lit.rating, lit.reading_status.to_string(), lit.is_dirty, lit.is_deleted, lit.version, lit.created_at, lit.updated_at,
                ],
            )?;

            debug!("数据库: 开始更新文献关联关系 (ID: {})", lit.id);
            Self::_set_authors(tx, &lit.id, &lit.authors)?;
            Self::_set_folders(tx, &lit.id, &lit.folder_ids)?;
            Self::_set_tags(tx, &lit.id, &lit.tags)?;
            Self::_set_attachments(tx, &lit.id, &lit.attachments)?;

            info!("数据库: 文献 {} 及其关联关系写入成功", lit.id);
            Ok(())
        })
    }

    pub fn update_literature_metadata(&self, lit: &Literature) -> Result<()> {
        debug!("数据库: 正在更新文献元数据 (ID: {})", lit.id);
        self.with_conn(|conn| {
            // 处理出版源
            let publication_id = Self::_set_publication(conn, lit.publication.as_ref())?;

            let rows = conn.execute(
                "UPDATE literatures SET
                    title = ?1, year = ?2, month = ?3, day = ?4, type = ?5, publication_id = ?6,
                    volume = ?7, issue = ?8, pages = ?9,
                    abstract_text = ?10, doi = ?11, arxiv_id = ?12, isbn = ?13, url = ?14, notes = ?15,
                    rating = ?16, updated_at = ?17, is_dirty = 1, version = version + 1
                WHERE id = ?18",
                params![
                    lit.title, lit.year, lit.month, lit.day,
                    serde_json::to_string(&lit.literature_type).unwrap_or_else(|_| "\"article\"".to_string()).trim_matches('"'),
                    publication_id,
                    lit.volume, lit.issue, lit.pages,
                    lit.abstract_text, lit.doi, lit.arxiv_id, lit.isbn, lit.url, lit.notes, lit.rating,
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), lit.id,
                ],
            )?;
            if rows == 0 {
                warn!("数据库: 更新文献元数据失败，未找到 ID 为 {} 的记录", lit.id);
            } else {
                debug!("数据库: 文献元数据更新完成 (ID: {})", lit.id);
            }
            Ok(())
        })
    }

    /// 更新文献笔记（过渡：写入新 literature_notes 表）
    pub fn update_literature_notes(&self, id: &str, notes: &str) -> Result<()> {
        debug!("数据库: 正在更新文献笔记 (ID: {id})");
        let existing = self.list_notes(id)?;
        if let Some(first) = existing.into_iter().next() {
            self.update_note(&first.id, None, Some(notes))?;
        } else {
            self.create_note(id, "笔记")?;
            self.update_note(&self.list_notes(id)?.first().unwrap().id, None, Some(notes))?;
        }
        Ok(())
    }

    pub fn update_reading_status(&self, id: &str, status: models::ReadingStatus) -> Result<()> {
        debug!("数据库: 正在更新文献阅读状态 (ID: {id}, status: {status:?})");
        self.with_conn(|conn| {
            let rows = conn.execute("UPDATE literatures SET reading_status = ?1, updated_at = ?2, is_dirty = 1, version = version + 1 WHERE id = ?3",
                params![status.to_string(), Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), id])?;
            if rows == 0 {
                warn!("数据库: 更新阅读状态失败，未找到 ID 为 {id} 的记录");
            } else {
                debug!("数据库: 文献阅读状态更新成功 (ID: {id})");
            }
            Ok(())
        })
    }

    pub fn get_folders_for_literature(&self, literature_id: &str) -> anyhow::Result<Vec<String>> {
        debug!("数据库: 正在获取文献所属文件夹 (ID: {literature_id})");
        let conn = self.conn.lock().map_err(|e| anyhow!("Lock error: {e}"))?;
        let mut stmt = conn.prepare(
            "SELECT folder_id FROM literature_folders WHERE literature_id = ? AND is_deleted = 0",
        )?;
        let rows = stmt.query_map([literature_id], |row: &Row| row.get::<_, String>(0))?;
        let mut folders = Vec::new();
        for folder in rows {
            folders.push(folder?);
        }
        debug!(
            "数据库: 成功获取 {} 个所属文件夹 (ID: {})",
            folders.len(),
            literature_id
        );
        Ok(folders)
    }

    pub fn set_literature_authors(&self, literature_id: &str, authors: &[Author]) -> Result<()> {
        info!(
            "数据库: 正在设置文献作者 (ID: {}, 作者数量: {})",
            literature_id,
            authors.len()
        );
        self.with_transaction(|tx| {
            Self::_set_authors(tx, literature_id, authors)?;
            tx.execute("UPDATE literatures SET updated_at = ?1, is_dirty = 1, version = version + 1 WHERE id = ?2",
                params![Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), literature_id])?;
            Ok(())
        })
    }

    pub fn set_literature_folders(&self, literature_id: &str, folder_ids: &[String]) -> Result<()> {
        info!(
            "数据库: 正在设置文献文件夹关联 (ID: {}, 文件夹数量: {})",
            literature_id,
            folder_ids.len()
        );
        self.with_transaction(|tx| {
            Self::_set_folders(tx, literature_id, folder_ids)?;
            tx.execute("UPDATE literatures SET updated_at = ?1, is_dirty = 1, version = version + 1 WHERE id = ?2",
                params![Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), literature_id])?;
            Ok(())
        })
    }

    pub fn set_literature_tags(&self, literature_id: &str, tags: &[String]) -> Result<()> {
        info!(
            "数据库: 正在设置文献标签 (ID: {}, 标签数量: {})",
            literature_id,
            tags.len()
        );
        self.with_transaction(|tx| {
            Self::_set_tags(tx, literature_id, tags)?;
            tx.execute("UPDATE literatures SET updated_at = ?1, is_dirty = 1, version = version + 1 WHERE id = ?2",
                params![Local::now().format("%Y-%m-%d %H:%M:%S").to_string(), literature_id])?;
            Ok(())
        })
    }

    /// 为文献添加单个标签（增量操作）
    pub fn add_tag_to_literature(&self, literature_id: &str, tag_name: &str) -> Result<()> {
        info!("数据库: 为文献添加标签 (ID: {literature_id}, 标签: {tag_name})");
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        self.with_transaction(|tx| {
            // 1. 确保标签存在（使用 create_tag 逻辑）
            let tag_id: Option<String> = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1 AND is_deleted = 0",
                [tag_name],
                |row| row.get(0)
            ).optional()?;

            let tag_id = if let Some(id) = tag_id {
                id
            } else {
                // 标签不存在，创建新标签
                let new_id = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO tags (id, name, color, is_dirty, is_deleted, version, created_at, updated_at)
                     VALUES (?1, ?2, '#4A90E2', 1, 0, 1, ?3, ?3)",
                    params![new_id, tag_name, now]
                )?;
                debug!("数据库: 创建新标签 '{tag_name}' (ID: {new_id})");
                new_id
            };

            // 2. 检查文献-标签关联是否存在
            let existing: Option<(bool, i32)> = tx.query_row(
                "SELECT is_deleted, version FROM literature_tags WHERE literature_id = ?1 AND tag_id = ?2",
                params![literature_id, tag_id],
                |row| Ok((row.get(0)?, row.get(1)?))
            ).optional()?;

            if let Some((is_deleted, version)) = existing {
                if is_deleted {
                    // 恢复已删除的关联
                    tx.execute(
                        "UPDATE literature_tags SET is_deleted = 0, is_dirty = 1, version = ?1, updated_at = ?2
                         WHERE literature_id = ?3 AND tag_id = ?4",
                        params![version + 1, now, literature_id, tag_id]
                    )?;
                    debug!("数据库: 恢复文献-标签关联 (LiteratureID: {literature_id}, Tag: {tag_name})");
                } else {
                    debug!("数据库: 文献已有该标签，跳过 (LiteratureID: {literature_id}, Tag: {tag_name})");
                }
            } else {
                // 创建新关联
                tx.execute(
                    "INSERT INTO literature_tags (literature_id, tag_id, is_dirty, is_deleted, version, updated_at)
                     VALUES (?1, ?2, 1, 0, 1, ?3)",
                    params![literature_id, tag_id, now]
                )?;
                debug!("数据库: 创建新文献-标签关联 (LiteratureID: {literature_id}, Tag: {tag_name})");
            }

            // 3. 更新文献的 updated_at 和 version
            tx.execute(
                "UPDATE literatures SET updated_at = ?1, is_dirty = 1, version = version + 1 WHERE id = ?2",
                params![now, literature_id]
            )?;

            Ok(())
        })
    }

    /// 从文献移除单个标签（软删除）
    pub fn remove_tag_from_literature(&self, literature_id: &str, tag_name: &str) -> Result<()> {
        info!("数据库: 从文献移除标签 (ID: {literature_id}, 标签: {tag_name})");
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        self.with_transaction(|tx| {
            // 1. 获取标签 ID
            let tag_id: Option<String> = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1",
                [tag_name],
                |row| row.get(0)
            ).optional()?;

            if let Some(tag_id) = tag_id {
                // 2. 软删除关联
                let rows = tx.execute(
                    "UPDATE literature_tags SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1
                     WHERE literature_id = ?2 AND tag_id = ?3 AND is_deleted = 0",
                    params![now, literature_id, tag_id]
                )?;

                if rows > 0 {
                    debug!("数据库: 成功移除文献-标签关联 (LiteratureID: {literature_id}, Tag: {tag_name})");

                    // 3. 更新文献的 updated_at 和 version
                    tx.execute(
                        "UPDATE literatures SET updated_at = ?1, is_dirty = 1, version = version + 1 WHERE id = ?2",
                        params![now, literature_id]
                    )?;
                } else {
                    warn!("数据库: 未找到需要删除的文献-标签关联 (LiteratureID: {literature_id}, Tag: {tag_name})");
                }
            } else {
                warn!("数据库: 标签不存在，无法移除 (Tag: {tag_name})");
            }

            Ok(())
        })
    }

    /// 获取文献的所有标签（包含 Tag 对象）
    pub fn get_literature_tags(&self, literature_id: &str) -> Result<Vec<models::Tag>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT t.id, t.name, t.color, t.created_at, t.updated_at, t.version, t.is_deleted
                 FROM tags t
                 JOIN literature_tags lt ON t.id = lt.tag_id
                 WHERE lt.literature_id = ?1 AND lt.is_deleted = 0 AND t.is_deleted = 0
                 ORDER BY t.name ASC",
            )?;

            let tag_iter = stmt.query_map([literature_id], |row| {
                Ok(Tag {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    version: row.get(5)?,
                    is_deleted: row.get(6)?,
                    is_dirty: false,
                })
            })?;

            let mut tags = Vec::new();
            for tag in tag_iter {
                tags.push(tag?);
            }

            Ok(tags)
        })
    }

    pub fn get_literature(&self, id: &str) -> Result<Option<Literature>> {
        debug!("数据库: 正在获取文献详情 (ID: {id})");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, title, year, month, day, type, volume, issue, pages, abstract_text, doi, arxiv_id, isbn, url, notes, rating, reading_status, is_dirty, is_deleted, version, created_at, updated_at, publication_id FROM literatures WHERE id = ?1")?;
            let mut rows = stmt.query([id])?;
            if let Some(row) = rows.next()? {
                let lit = Self::_map_literature_row(conn, row)?;
                debug!("数据库: 成功获取文献详情: {}", lit.title);
                Ok(Some(lit))
            } else {
                debug!("数据库: 未找到 ID 为 {id} 的文献记录");
                Ok(None)
            }
        })
    }

    pub fn get_all_literatures(&self) -> Result<Vec<Literature>> {
        debug!("数据库: 正在获取所有未删除的文献");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, title, year, month, day, type, volume, issue, pages, abstract_text, doi, arxiv_id, isbn, url, notes, rating, reading_status, is_dirty, is_deleted, version, created_at, updated_at, publication_id FROM literatures WHERE is_deleted = 0 ORDER BY created_at DESC")?;
            let lit_iter = stmt.query_map([], |row| Self::_map_literature_row(conn, row))?;
            let mut literatures = Vec::new();
            for lit in lit_iter { literatures.push(lit?); }
            debug!("数据库: 成功加载 {} 条文献记录", literatures.len());
            Ok(literatures)
        })
    }

    pub fn get_literatures_by_folder(&self, folder_id: &str) -> Result<Vec<Literature>> {
        debug!("数据库: 正在获取文件夹中的文献 (FolderID: {folder_id})");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT l.id, l.title, l.year, l.month, l.day, l.type, l.volume, l.issue, l.pages, l.abstract_text, l.doi, l.arxiv_id, l.isbn, l.url, l.notes, l.rating, l.reading_status, l.is_dirty, l.is_deleted, l.version, l.created_at, l.updated_at, l.publication_id FROM literatures l JOIN literature_folders lf ON l.id = lf.literature_id WHERE lf.folder_id = ?1 AND l.is_deleted = 0 AND lf.is_deleted = 0 ORDER BY l.created_at DESC")?;
            let lit_iter = stmt.query_map([folder_id], |row| Self::_map_literature_row(conn, row))?;
            let mut literatures = Vec::new();
            for lit in lit_iter { literatures.push(lit?); }
            debug!("数据库: 文件夹 {} 中共有 {} 条文献记录", folder_id, literatures.len());
            Ok(literatures)
        })
    }

    pub fn get_uncategorized_literatures(&self) -> Result<Vec<Literature>> {
        debug!("数据库: 正在获取未分类文献");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT l.id, l.title, l.year, l.month, l.day, l.type, l.volume, l.issue, l.pages, l.abstract_text, l.doi, l.arxiv_id, l.isbn, l.url, l.notes, l.rating, l.reading_status, l.is_dirty, l.is_deleted, l.version, l.created_at, l.updated_at, l.publication_id FROM literatures l LEFT JOIN literature_folders lf ON l.id = lf.literature_id AND lf.is_deleted = 0 WHERE lf.literature_id IS NULL AND l.is_deleted = 0 ORDER BY l.created_at DESC")?;
            let lit_iter = stmt.query_map([], |row| Self::_map_literature_row(conn, row))?;
            let mut literatures = Vec::new();
            for lit in lit_iter { literatures.push(lit?); }
            debug!("数据库: 共有 {} 条未分类文献", literatures.len());
            Ok(literatures)
        })
    }

    pub fn delete_literature(&self, id: &str) -> Result<()> {
        info!("数据库: 准备软删除文献记录: {id}");
        self.with_conn(|conn| {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let rows = conn.execute("UPDATE literatures SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE id = ?2", params![now, id])?;
            if rows > 0 {
                info!("数据库: 文献 {id} 已成功标记为软删除");
            } else {
                warn!("数据库: 软删除文献失败，找不到 ID 为 {id} 的记录");
            }
            Ok(())
        })
    }

    pub fn get_dirty_literatures(&self) -> Result<Vec<Literature>> {
        debug!("数据库: 正在获取所有标记为脏数据的文献");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT id, title, year, month, day, type, volume, issue, pages, abstract_text, doi, arxiv_id, isbn, url, notes, rating, reading_status, is_dirty, is_deleted, version, created_at, updated_at, publication_id FROM literatures WHERE is_dirty = 1")?;
            let lit_iter = stmt.query_map([], |row| Self::_map_literature_row(conn, row))?;
            let mut literatures = Vec::new();
            for lit in lit_iter { literatures.push(lit?); }
            debug!("数据库: 成功加载 {} 条脏数据文献记录", literatures.len());
            Ok(literatures)
        })
    }

    pub fn mark_literature_synced(&self, id: &str) -> Result<()> {
        debug!("数据库: 正在标记文献已同步 (ID: {id})");
        self.with_conn(|conn| {
            let rows = conn.execute("UPDATE literatures SET is_dirty = 0 WHERE id = ?1", [id])?;
            if rows == 0 {
                warn!("数据库: 标记同步失败，未找到 ID 为 {id} 的记录");
            } else {
                debug!("数据库: 文献 {id} 已标记为同步状态");
            }
            Ok(())
        })
    }

    pub fn purge_synced_deletions(&self) -> Result<usize> {
        info!("数据库: 正在清理已同步的删除记录");
        self.with_transaction(|tx| {
            let mut total = 0;
            total += tx.execute(
                "DELETE FROM literatures WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            total += tx.execute(
                "DELETE FROM literature_authors WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            total += tx.execute(
                "DELETE FROM literature_folders WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            total += tx.execute(
                "DELETE FROM literature_tags WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            info!("数据库: 清理完成，共删除 {total} 条已同步的物理记录");
            Ok(total)
        })
    }

    pub fn merge_remote_literature(&self, remote_lit: Literature) -> Result<Option<Literature>> {
        info!(
            "数据库: 正在合并远程文献记录 (ID: {}, Title: {})",
            remote_lit.id, remote_lit.title
        );
        self.with_conn(|conn| {
            let local_info: Option<(i32, bool)> = conn
                .query_row(
                    "SELECT version, is_dirty FROM literatures WHERE id = ?1",
                    [&remote_lit.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((local_version, is_dirty)) = local_info {
                if remote_lit.version > local_version && !is_dirty {
                    info!(
                        "数据库: 远程版本较新且本地无修改，准备覆盖 (ID: {}, {} -> {})",
                        remote_lit.id, local_version, remote_lit.version
                    );
                    self._insert_literature_internal(conn, &remote_lit)?;
                    Ok(None)
                } else if remote_lit.version == local_version && !is_dirty {
                    debug!(
                        "数据库: 版本一致且本地无修改，仅更新时间戳 (ID: {})",
                        remote_lit.id
                    );
                    conn.execute(
                        "UPDATE literatures SET updated_at = ?1, is_dirty = 0 WHERE id = ?2",
                        params![remote_lit.updated_at, remote_lit.id],
                    )?;
                    Ok(None)
                } else {
                    warn!(
                        "数据库: 发现合并冲突 (ID: {}) 本地版本: {}, 远程版本: {}, 本地Dirty: {}",
                        remote_lit.id, local_version, remote_lit.version, is_dirty
                    );
                    Ok(Some(remote_lit))
                }
            } else {
                info!(
                    "数据库: 本地不存在该文献，准备插入远程记录 (ID: {})",
                    remote_lit.id
                );
                self._insert_literature_internal(conn, &remote_lit)?;
                Ok(None)
            }
        })
    }

    fn _insert_literature_internal(&self, conn: &Connection, lit: &Literature) -> Result<()> {
        debug!("数据库: 正在执行内部文献插入/更新 (ID: {})", lit.id);
        // 处理出版源
        let publication_id = Self::_set_publication(conn, lit.publication.as_ref())?;

        conn.execute(
            "INSERT OR REPLACE INTO literatures (id, title, year, month, day, type, publication_id, volume, issue, pages, abstract_text, doi, arxiv_id, isbn, url, notes, rating, reading_status, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 0, ?19, ?20, ?21, ?22)",
            params![lit.id, lit.title, lit.year, lit.month, lit.day, serde_json::to_string(&lit.literature_type).unwrap_or_default().trim_matches('"'), publication_id, lit.volume, lit.issue, lit.pages, lit.abstract_text, lit.doi, lit.arxiv_id, lit.isbn, lit.url, lit.notes, lit.rating, lit.reading_status.to_string(), lit.is_deleted, lit.version, lit.created_at, lit.updated_at],
        )?;
        Ok(())
    }

    fn _map_literature_row(conn: &Connection, row: &Row) -> Result<Literature> {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        debug!("数据库: 正在从数据行映射文献对象 (ID: {id}, Title: {title})");
        let lit_type_str: String = row.get(5)?;
        let lit_type: LiteratureType =
            serde_json::from_str(&format!("\"{lit_type_str}\"")).unwrap_or(LiteratureType::Article);
        let mut lit = create_literature(id.clone(), title, lit_type);
        lit.year = row.get(2)?;
        lit.month = row.get(3)?;
        lit.day = row.get(4)?;

        lit.volume = row.get(6)?;
        lit.issue = row.get(7)?;
        lit.pages = row.get(8)?;
        lit.abstract_text = row.get(9)?;
        lit.doi = row.get(10)?;
        lit.arxiv_id = row.get(11)?;
        lit.isbn = row.get(12)?;
        lit.url = row.get(13)?;
        lit.notes = row.get(14)?;
        lit.rating = row.get(15)?;
        lit.reading_status = match row.get::<_, String>(16) {
            Ok(s) => match s.as_str() {
                "Reading" => models::ReadingStatus::Reading,
                "Read" => models::ReadingStatus::Read,
                _ => models::ReadingStatus::Unread,
            },
            Err(_) => models::ReadingStatus::Unread,
        };
        lit.is_dirty = row.get(17)?;
        lit.is_deleted = row.get(18)?;
        lit.version = row.get(19)?;
        lit.created_at = row.get(20)?;
        lit.updated_at = row.get(21)?;

        // 加载出版源信息 (在 SELECT 列表的最后)
        let publication_id: Option<String> = row.get(22).unwrap_or(None);
        if let Some(ref pub_id) = publication_id {
            lit.publication = Self::_get_publication(conn, pub_id)?;
        }

        debug!("数据库: 正在加载文献关联信息 (ID: {id})");
        let mut auth_stmt = conn.prepare("SELECT a.* FROM authors a JOIN literature_authors la ON a.id = la.author_id WHERE la.literature_id = ?1 AND la.is_deleted = 0 ORDER BY la.sort_order")?;
        lit.authors = auth_stmt
            .query_map([id.clone()], |a_row| {
                Ok(Author {
                    id: a_row.get(0)?,
                    first_name: a_row.get(1)?,
                    last_name: a_row.get(2)?,
                    middle_name: a_row.get(3)?,
                    is_dirty: a_row.get(4)?,
                    is_deleted: a_row.get(5)?,
                    version: a_row.get(6)?,
                    created_at: a_row.get(7)?,
                    updated_at: a_row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        debug!("数据库: 加载了 {} 位作者 (ID: {})", lit.authors.len(), id);

        let mut folder_stmt = conn.prepare(
            "SELECT folder_id FROM literature_folders WHERE literature_id = ?1 AND is_deleted = 0",
        )?;
        lit.folder_ids = folder_stmt
            .query_map([id.clone()], |f_row| f_row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        debug!(
            "数据库: 加载了 {} 个文件夹关联 (ID: {})",
            lit.folder_ids.len(),
            id
        );

        let mut tag_stmt = conn.prepare("SELECT t.name FROM tags t JOIN literature_tags lt ON t.id = lt.tag_id WHERE lt.literature_id = ?1 AND lt.is_deleted = 0")?;
        lit.tags = tag_stmt
            .query_map([id.clone()], |t_row| t_row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        debug!("数据库: 加载了 {} 个标签 (ID: {})", lit.tags.len(), id);

        let mut att_stmt = conn.prepare("SELECT id, literature_id, file_path, file_name, file_size, mime_type, etag, is_main, is_dirty, is_deleted, version, created_at, updated_at FROM attachments WHERE literature_id = ?1 AND is_deleted = 0")?;
        lit.attachments = att_stmt
            .query_map([id.clone()], |row| {
                Ok(Attachment {
                    id: row.get(0)?,
                    literature_id: row.get(1)?,
                    file_path: row.get(2)?,
                    file_name: row.get(3)?,
                    file_size: row.get(4)?,
                    mime_type: row.get(5)?,
                    etag: row.get(6)?,
                    is_main: row.get(7)?,
                    is_dirty: row.get(8)?,
                    is_deleted: row.get(9)?,
                    version: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })?
            .collect::<Result<Vec<Attachment>>>()?;
        info!(
            "数据库: 文献 {} 加载了 {} 个附件",
            id,
            lit.attachments.len()
        );

        Ok(lit)
    }

    /// 处理出版源：查找已存在的或创建新的
    /// 返回出版源 ID（如果有）
    fn _set_publication(
        conn: &Connection,
        publication: Option<&Publication>,
    ) -> Result<Option<String>> {
        let Some(pub_data) = publication else {
            return Ok(None);
        };

        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let pub_type_str = pub_data.publication_type.to_string();

        // 首先查找是否已存在同名同类型的出版源（优先选择未删除的）
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM publications
                 WHERE LOWER(name) = LOWER(?1) AND publication_type = ?2
                 ORDER BY is_deleted ASC, version DESC
                 LIMIT 1",
                params![pub_data.name, pub_type_str],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(existing_id) = existing_id {
            // 合并：更新现有记录的元数据（如 rank 信息）
            info!(
                "数据库: 找到已存在的出版源 '{}' (ID: {}), 执行合并",
                pub_data.name, existing_id
            );

            // 仅更新非空的 rank 字段
            if pub_data.ccf_rank.is_some()
                || pub_data.jcr_rank.is_some()
                || pub_data.cas_rank.is_some()
            {
                conn.execute(
                    "UPDATE publications SET
                        ccf_rank = COALESCE(?1, ccf_rank),
                        jcr_rank = COALESCE(?2, jcr_rank),
                        cas_rank = COALESCE(?3, cas_rank),
                        abbreviation = COALESCE(?4, abbreviation),
                        issn = COALESCE(?5, issn),
                        isbn = COALESCE(?6, isbn),
                        publisher = COALESCE(?7, publisher),
                        updated_at = ?8,
                        is_dirty = 1,
                        version = version + 1
                    WHERE id = ?9",
                    params![
                        pub_data.ccf_rank,
                        pub_data.jcr_rank,
                        pub_data.cas_rank,
                        pub_data.abbreviation,
                        pub_data.issn,
                        pub_data.isbn,
                        pub_data.publisher,
                        now,
                        existing_id
                    ],
                )?;
            }

            Ok(Some(existing_id))
        } else {
            // 创建新的出版源
            let new_id = Uuid::new_v4().to_string();
            info!(
                "数据库: 创建新的出版源 '{}' (ID: {})",
                pub_data.name, new_id
            );

            conn.execute(
                "INSERT INTO publications (id, name, publication_type, abbreviation, issn, isbn, publisher, ccf_rank, jcr_rank, cas_rank, is_dirty, is_deleted, version, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, 0, 1, ?11, ?12)",
                params![
                    new_id, pub_data.name, pub_type_str,
                    pub_data.abbreviation, pub_data.issn, pub_data.isbn, pub_data.publisher,
                    pub_data.ccf_rank, pub_data.jcr_rank, pub_data.cas_rank,
                    now, now
                ],
            )?;

            Ok(Some(new_id))
        }
    }

    /// 根据 ID 获取出版源信息
    fn _get_publication(conn: &Connection, id: &str) -> Result<Option<Publication>> {
        let pub_opt: Option<Publication> = conn
            .query_row(
                "SELECT id, name, publication_type, abbreviation, issn, isbn, publisher, ccf_rank, jcr_rank, cas_rank, is_dirty, is_deleted, version, created_at, updated_at
                 FROM publications WHERE id = ?1",
                params![id],
                |row| {
                    let pub_type_str: String = row.get(2)?;
                    let pub_type = match pub_type_str.as_str() {
                        "Conference" => PublicationType::Conference,
                        "Book" => PublicationType::Book,
                        _ => PublicationType::Journal,
                    };
                    Ok(Publication {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        publication_type: pub_type,
                        abbreviation: row.get(3)?,
                        issn: row.get(4)?,
                        isbn: row.get(5)?,
                        publisher: row.get(6)?,
                        ccf_rank: row.get(7)?,
                        jcr_rank: row.get(8)?,
                        cas_rank: row.get(9)?,
                        is_dirty: row.get(10)?,
                        is_deleted: row.get(11)?,
                        version: row.get(12)?,
                        created_at: row.get(13)?,
                        updated_at: row.get(14)?,
                    })
                },
            )
            .optional()?;

        Ok(pub_opt)
    }

    fn _set_authors(conn: &Connection, literature_id: &str, authors: &[Author]) -> Result<()> {
        debug!(
            "数据库: 正在更新文献作者关联关系 (ID: {}, 作者数: {})",
            literature_id,
            authors.len()
        );
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare("SELECT author_id, is_deleted, version FROM literature_authors WHERE literature_id = ?1")?;
        let current_relations: Vec<(String, bool, i32)> = stmt
            .query_map([literature_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut canonical_authors = Vec::new();
        for author in authors {
            let mut stmt = conn.prepare("SELECT id FROM authors WHERE first_name = ?1 AND last_name = ?2 AND COALESCE(middle_name, '') = ?3 AND is_deleted = 0 LIMIT 1")?;
            let middle_name = author.middle_name.as_deref().unwrap_or("");
            let existing_id: Option<String> = stmt
                .query_row(
                    params![author.first_name, author.last_name, middle_name],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(id) = existing_id {
                let mut a = author.clone();
                if a.id != id {
                    info!(
                        "数据库: 发现同名作者 '{} {}', 执行合并: 将临时 ID {} 修正为现有 ID {}",
                        author.first_name, author.last_name, a.id, id
                    );
                    a.id = id;
                }
                canonical_authors.push(a);
            } else {
                info!(
                    "数据库: 未发现同名作者, 使用新解析的 ID: {} (姓名: {} {})",
                    author.id, author.first_name, author.last_name
                );
                canonical_authors.push(author.clone());
            }
        }

        let target_ids: Vec<String> = canonical_authors.iter().map(|a| a.id.clone()).collect();
        let mut deleted_count = 0;
        for (auth_id, is_deleted, version) in &current_relations {
            if !target_ids.contains(auth_id) && !is_deleted {
                conn.execute("UPDATE literature_authors SET is_deleted = 1, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND author_id = ?4", params![version + 1, now, literature_id, auth_id])?;
                deleted_count += 1;
            }
        }
        if deleted_count > 0 {
            debug!("数据库: 标记删除了 {deleted_count} 条过时的作者关联 (ID: {literature_id})");
        }

        for (i, author) in canonical_authors.iter().enumerate() {
            info!(
                "数据库: 正在关联作者, 文献ID: {}, 作者ID: {}, 姓名: {} {}, 排序: {}",
                literature_id, author.id, author.first_name, author.last_name, i
            );
            conn.execute("INSERT OR REPLACE INTO authors (id, first_name, last_name, middle_name, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)", params![author.id, author.first_name, author.last_name, author.middle_name, author.is_dirty, author.is_deleted, author.version, author.created_at, author.updated_at])?;
            let existing = current_relations.iter().find(|(id, _, _)| id == &author.id);
            match existing {
                Some((_, is_deleted, version)) => {
                    conn.execute("UPDATE literature_authors SET sort_order = ?1, is_deleted = 0, is_dirty = 1, version = ?2, updated_at = ?3 WHERE literature_id = ?4 AND author_id = ?5", params![i as i32, if *is_deleted { version + 1 } else { *version }, now, literature_id, author.id])?;
                }
                None => {
                    conn.execute("INSERT INTO literature_authors (literature_id, author_id, sort_order, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, ?3, 1, 0, 1, ?4)", params![literature_id, author.id, i as i32, now])?;
                }
            }
        }
        Ok(())
    }

    fn _set_folders(conn: &Connection, literature_id: &str, folder_ids: &[String]) -> Result<()> {
        debug!(
            "数据库: 正在更新文献文件夹关联 (ID: {}, 文件夹数: {})",
            literature_id,
            folder_ids.len()
        );
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare("SELECT folder_id, is_deleted, version FROM literature_folders WHERE literature_id = ?1")?;
        let current_relations: Vec<(String, bool, i32)> = stmt
            .query_map([literature_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut deleted_count = 0;
        for (f_id, is_deleted, version) in &current_relations {
            if !folder_ids.contains(f_id) && !is_deleted {
                conn.execute("UPDATE literature_folders SET is_deleted = 1, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND folder_id = ?4", params![version + 1, now, literature_id, f_id])?;
                deleted_count += 1;
            }
        }
        if deleted_count > 0 {
            debug!("数据库: 标记删除了 {deleted_count} 条过时的文件夹关联 (ID: {literature_id})");
        }

        let mut seen = std::collections::HashSet::new();
        for fid in folder_ids.iter().filter(|fid| seen.insert(*fid)) {
            let existing = current_relations.iter().find(|(id, _, _)| id == fid);
            if let Some((_, is_deleted, version)) = existing {
                if *is_deleted {
                    debug!(
                        "数据库: 恢复文件夹关联 (LiteratureID: {literature_id}, FolderID: {fid})"
                    );
                    conn.execute("UPDATE literature_folders SET is_deleted = 0, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND folder_id = ?4", params![version + 1, now, literature_id, fid])?;
                }
            } else {
                debug!("数据库: 创建新文件夹关联 (LiteratureID: {literature_id}, FolderID: {fid})");
                conn.execute("INSERT INTO literature_folders (literature_id, folder_id, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, 1, 0, 1, ?3)", params![literature_id, fid, now])?;
            }
        }
        Ok(())
    }

    fn _set_tags(conn: &Connection, literature_id: &str, tags: &[String]) -> Result<()> {
        debug!(
            "数据库: 正在更新文献标签关联 (ID: {}, 标签数: {})",
            literature_id,
            tags.len()
        );
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn.prepare("SELECT t.name, lt.is_deleted, lt.version FROM tags t JOIN literature_tags lt ON t.id = lt.tag_id WHERE lt.literature_id = ?1")?;
        let current_relations: Vec<(String, bool, i32)> = stmt
            .query_map([literature_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut deleted_count = 0;
        for (tag_name, is_deleted, version) in &current_relations {
            if !tags.contains(tag_name) && !is_deleted {
                let tag_id: String =
                    conn.query_row("SELECT id FROM tags WHERE name = ?1", [tag_name], |row| {
                        row.get(0)
                    })?;
                conn.execute("UPDATE literature_tags SET is_deleted = 1, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND tag_id = ?4", params![version + 1, now, literature_id, tag_id])?;
                deleted_count += 1;
            }
        }
        if deleted_count > 0 {
            debug!("数据库: 标记删除了 {deleted_count} 条过时的标签关联 (ID: {literature_id})");
        }

        for tag_name in tags {
            conn.execute(
                "INSERT OR IGNORE INTO tags (id, name, color, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, '#808080', 1, 0, 1, ?3, ?3)",
                params![Uuid::new_v4().to_string(), tag_name, now],
            )?;
            let tag_id: String =
                conn.query_row("SELECT id FROM tags WHERE name = ?1", [tag_name], |row| {
                    row.get(0)
                })?;
            let existing = current_relations
                .iter()
                .find(|(name, _, _)| name == tag_name);
            if let Some((_, is_deleted, version)) = existing {
                if *is_deleted {
                    debug!("数据库: 恢复标签关联 (LiteratureID: {literature_id}, Tag: {tag_name})");
                    conn.execute("UPDATE literature_tags SET is_deleted = 0, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND tag_id = ?4", params![version + 1, now, literature_id, tag_id])?;
                }
            } else {
                debug!("数据库: 创建新标签关联 (LiteratureID: {literature_id}, Tag: {tag_name})");
                conn.execute("INSERT INTO literature_tags (literature_id, tag_id, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, 1, 0, 1, ?3)", params![literature_id, tag_id, now])?;
            }
        }
        Ok(())
    }

    fn _set_attachments(
        conn: &Connection,
        literature_id: &str,
        attachments: &[Attachment],
    ) -> Result<()> {
        debug!(
            "数据库: 正在更新文献附件关联 (ID: {}, 附件数: {})",
            literature_id,
            attachments.len()
        );
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = conn
            .prepare("SELECT id, is_deleted, version FROM attachments WHERE literature_id = ?1 AND is_deleted = 0")?;
        let current_relations: Vec<(String, bool, i32)> = stmt
            .query_map([literature_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut deleted_count = 0;
        for (att_id, is_deleted, version) in &current_relations {
            if !attachments.iter().any(|a| &a.id == att_id) && !is_deleted {
                conn.execute("UPDATE attachments SET is_deleted = 1, is_dirty = 1, version = ?1, updated_at = ?2 WHERE id = ?3", params![version + 1, now, att_id])?;
                deleted_count += 1;
            }
        }
        if deleted_count > 0 {
            debug!("数据库: 标记删除了 {deleted_count} 条过时的附件关联 (ID: {literature_id})");
        }

        for att in attachments {
            let existing = current_relations.iter().find(|(id, _, _)| id == &att.id);
            if let Some((_, is_deleted, version)) = existing {
                // 无论附件是否被删除,都更新所有字段以确保数据一致性
                debug!(
                    "数据库: 更新现有附件 (ID: {}, FileName: {}, is_deleted: {})",
                    att.id, att.file_name, is_deleted
                );
                conn.execute(
                    "UPDATE attachments SET file_path = ?1, file_name = ?2, file_size = ?3, mime_type = ?4, etag = ?5, is_main = ?6, is_deleted = 0, is_dirty = 1, version = ?7, updated_at = ?8 WHERE id = ?9",
                    params![att.file_path, att.file_name, att.file_size, att.mime_type, att.etag, att.is_main, version + 1, now, att.id]
                )?;
            } else {
                debug!(
                    "数据库: 创建新附件关联 (ID: {}, FileName: {})",
                    att.id, att.file_name
                );
                match conn.execute("INSERT INTO attachments (id, literature_id, file_path, file_name, file_size, mime_type, etag, is_main, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, 0, 1, ?9, ?10)", params![att.id, literature_id, att.file_path, att.file_name, att.file_size, att.mime_type, att.etag, att.is_main, now, now]) {
                    Ok(_) => info!("数据库: 成功插入附件 {} 到文献 {}", att.file_name, literature_id),
                    Err(e) => error!("数据库: 插入附件失败: {e}"),
                }
            }
        }
        Ok(())
    }

    pub fn get_dirty_relations(
        &self,
    ) -> Result<(
        Vec<AuthorRelationTuple>,
        Vec<CommonRelationTuple>,
        Vec<CommonRelationTuple>,
    )> {
        debug!("数据库: 正在获取所有标记为脏数据的文献关联关系");
        self.with_conn(|conn| {
            let mut authors = Vec::new(); let mut folders = Vec::new(); let mut tags = Vec::new();
            let mut stmt = conn.prepare("SELECT literature_id, author_id, sort_order, is_deleted, version FROM literature_authors WHERE is_dirty = 1")?;
            let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, Some(row.get::<_, i32>(2)?), row.get::<_, bool>(3)?, row.get::<_, i32>(4)?)))?;
            for r in rows { authors.push(r?); }

            let mut stmt = conn.prepare("SELECT literature_id, folder_id, is_deleted, version FROM literature_folders WHERE is_dirty = 1")?;
            let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?, row.get::<_, i32>(3)?)))?;
            for r in rows { folders.push(r?); }

            let mut stmt = conn.prepare("SELECT literature_id, tag_id, is_deleted, version FROM literature_tags WHERE is_dirty = 1")?;
            let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, bool>(2)?, row.get::<_, i32>(3)?)))?;
            for r in rows { tags.push(r?); }

            debug!("数据库: 获取到脏数据关联: authors={}, folders={}, tags={}", authors.len(), folders.len(), tags.len());
            Ok((authors, folders, tags))
        })
    }

    pub fn mark_relation_synced(&self, table: &str, lit_id: &str, target_id: &str) -> Result<()> {
        debug!(
            "数据库: 正在标记关联关系已同步 (Table: {table}, LiteratureID: {lit_id}, TargetID: {target_id})"
        );
        let id_col = match table {
            "literature_authors" => "author_id",
            "literature_folders" => "folder_id",
            "literature_tags" => "tag_id",
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        let sql =
            format!("UPDATE {table} SET is_dirty = 0 WHERE literature_id = ?1 AND {id_col} = ?2");
        self.with_conn(|conn| {
            let rows = conn.execute(&sql, [lit_id, target_id])?;
            if rows == 0 {
                warn!("数据库: 标记关联同步失败，未找到匹配记录 (Table: {table}, LiteratureID: {lit_id}, TargetID: {target_id})");
            } else {
                debug!("数据库: 关联关系标记同步成功");
            }
            Ok(())
        })
    }

    pub fn merge_remote_relation(
        &self,
        table: &str,
        lit_id: String,
        target_id: String,
        sort_order: Option<i32>,
        is_deleted: bool,
        version: i32,
    ) -> Result<()> {
        info!(
            "数据库: 正在合并远程文献关联 (Table: {table}, LiteratureID: {lit_id}, TargetID: {target_id}, Version: {version})"
        );
        let id_col = match table {
            "literature_authors" => "author_id",
            "literature_folders" => "folder_id",
            "literature_tags" => "tag_id",
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        self.with_conn(|conn| {
            let sql = format!("SELECT version, is_dirty FROM {table} WHERE literature_id = ?1 AND {id_col} = ?2");
            let local_info: Option<(i32, bool)> = conn.query_row(&sql, [&lit_id, &target_id], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
            if let Some((local_v, is_dirty)) = local_info {
                if version > local_v && !is_dirty {
                    info!("数据库: 远程关联版本较新且本地无修改，准备覆盖 (Table: {table}, {local_v} -> {version})");
                    self._upsert_relation_internal(conn, table, RelationUpsertData {
                        lit_id: lit_id.clone(),
                        target_id: target_id.clone(),
                        sort_order,
                        is_deleted,
                        version,
                    })?;
                }
                else if version == local_v && !is_dirty {
                    debug!("数据库: 关联版本一致且本地无修改，仅清除 Dirty 标记");
                    let up_sql = format!("UPDATE {table} SET is_dirty = 0 WHERE literature_id = ?1 AND {id_col} = ?2");
                    conn.execute(&up_sql, [&lit_id, &target_id])?;
                } else {
                    warn!("数据库: 发现关联合并冲突 (Table: {table}, LiteratureID: {lit_id}) 本地版本: {local_v}, 远程版本: {version}, Dirty: {is_dirty}");
                }
            } else {
                info!("数据库: 本地不存在该关联，准备插入远程记录 (Table: {table})");
                self._upsert_relation_internal(conn, table, RelationUpsertData {
                    lit_id,
                    target_id,
                    sort_order,
                    is_deleted,
                    version,
                })?;
            }
            Ok(())
        })
    }

    /// 合并文献关联关系（将源文献的关联迁移到目标文献）
    pub fn merge_literature_relations(&self, source_id: &str, target_id: &str) -> Result<()> {
        info!("数据库: 开始合并文献关联关系 ({source_id} -> {target_id})");

        // 1. 合并标签
        if let Err(e) = self._merge_literature_tags(source_id, target_id) {
            error!("合并标签失败: {e}");
        }

        // 2. 合并文件夹
        if let Err(e) = self._merge_literature_folders(source_id, target_id) {
            error!("合并文件夹失败: {e}");
        }

        // 3. 合并附件
        if let Err(e) = self.merge_attachments(source_id, target_id) {
            error!("合并附件失败: {e}");
        }

        // 4. 合并引用关系
        if let Err(e) = self.merge_citations(source_id, target_id) {
            error!("合并引用关系失败: {e}");
        }

        // 5. 合并元数据 (Rating, ReadingStatus, Notes)
        self.with_conn(|conn| {
            // 获取源文献和目标文献的相关字段
            let mut stmt = conn.prepare("SELECT rating, reading_status, notes FROM literatures WHERE id = ?1")?;

            let source_data = stmt.query_row([source_id], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            }).optional()?;

            let target_data = stmt.query_row([target_id], |row| {
                Ok((
                    row.get::<_, i32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            }).optional()?;

            if let (Some((s_rating, s_status, s_notes)), Some((t_rating, t_status, t_notes))) = (source_data, target_data) {
                let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

                // 评分：取最高
                let new_rating = std::cmp::max(s_rating, t_rating);

                // 阅读状态：优先级 Read > Reading > Unread
                fn status_score(s: &str) -> i32 {
                    match s {
                        "Read" => 3,
                        "Reading" => 2,
                        _ => 1,
                    }
                }
                let new_status = if status_score(&s_status) > status_score(&t_status) { s_status } else { t_status };

                // 笔记：合并
                let new_notes = match (s_notes, t_notes) {
                    (Some(s), Some(t)) => {
                        if s.trim().is_empty() { Some(t) }
                        else if t.trim().is_empty() { Some(s) }
                        else { Some(format!("{t}\n\n---\n\n{s}")) }
                    },
                    (Some(s), None) => Some(s),
                    (None, Some(t)) => Some(t),
                    (None, None) => None,
                };

                // 更新目标文献
                conn.execute(
                    "UPDATE literatures SET rating = ?1, reading_status = ?2, notes = ?3, is_dirty = 1, version = version + 1, updated_at = ?4 WHERE id = ?5",
                    params![new_rating, new_status, new_notes, now, target_id]
                )?;
            }

            Ok(())
        })
    }

    fn _upsert_relation_internal(
        &self,
        conn: &Connection,
        table: &str,
        data: RelationUpsertData,
    ) -> Result<()> {
        debug!(
            "数据库: 正在执行内部关联 Upsert (Table: {}, LiteratureID: {}, TargetID: {})",
            table, data.lit_id, data.target_id
        );
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        match table {
            "literature_authors" => {
                conn.execute("INSERT OR REPLACE INTO literature_authors (literature_id, author_id, sort_order, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6)", params![data.lit_id, data.target_id, data.sort_order.unwrap_or(0), data.is_deleted, data.version, now])?;
            }
            "literature_folders" => {
                conn.execute("INSERT OR REPLACE INTO literature_folders (literature_id, folder_id, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, 0, ?3, ?4, ?5)", params![data.lit_id, data.target_id, data.is_deleted, data.version, now])?;
            }
            "literature_tags" => {
                conn.execute("INSERT OR REPLACE INTO literature_tags (literature_id, tag_id, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, 0, ?3, ?4, ?5)", params![data.lit_id, data.target_id, data.is_deleted, data.version, now])?;
            }
            _ => {}
        }
        Ok(())
    }

    /// 内部方法：合并文献标签关联
    fn _merge_literature_tags(&self, source_id: &str, target_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            // 找出源文献的所有标签
            let mut stmt = conn.prepare("SELECT tag_id, version FROM literature_tags WHERE literature_id = ?1 AND is_deleted = 0")?;
            let tag_relations: Vec<(String, i32)> = stmt.query_map([source_id], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<Vec<_>>>()?;

            for (tag_id, version) in tag_relations {
                // 检查目标文献是否已有关联
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM literature_tags WHERE literature_id = ?1 AND tag_id = ?2)",
                    [target_id, &tag_id],
                    |row| row.get(0)
                )?;

                if exists {
                    // 如果已存在关联，确保它是未删除状态
                    conn.execute(
                        "UPDATE literature_tags SET is_deleted = 0, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE literature_id = ?2 AND tag_id = ?3",
                        params![now, target_id, tag_id]
                    )?;
                } else {
                    // 插入新关联
                    conn.execute(
                        "INSERT INTO literature_tags (literature_id, tag_id, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, 1, 0, 1, ?3)",
                        params![target_id, tag_id, now]
                    )?;
                }
                // 软删除源文献关联
                conn.execute(
                    "UPDATE literature_tags SET is_deleted = 1, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND tag_id = ?4",
                    params![version + 1, now, source_id, tag_id]
                )?;
            }
            Ok(())
        })
    }

    /// 内部方法：合并文献文件夹关联
    fn _merge_literature_folders(&self, source_id: &str, target_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let mut stmt = conn.prepare("SELECT folder_id, version FROM literature_folders WHERE literature_id = ?1 AND is_deleted = 0")?;
            let folder_relations: Vec<(String, i32)> = stmt.query_map([source_id], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<Vec<_>>>()?;

            for (folder_id, version) in folder_relations {
                let exists: bool = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM literature_folders WHERE literature_id = ?1 AND folder_id = ?2)",
                    [target_id, &folder_id],
                    |row| row.get(0)
                )?;

                if exists {
                    conn.execute(
                        "UPDATE literature_folders SET is_deleted = 0, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE literature_id = ?2 AND folder_id = ?3",
                        params![now, target_id, folder_id]
                    )?;
                } else {
                    conn.execute(
                        "INSERT INTO literature_folders (literature_id, folder_id, is_dirty, is_deleted, version, updated_at) VALUES (?1, ?2, 1, 0, 1, ?3)",
                        params![target_id, folder_id, now]
                    )?;
                }
                conn.execute(
                    "UPDATE literature_folders SET is_deleted = 1, is_dirty = 1, version = ?1, updated_at = ?2 WHERE literature_id = ?3 AND folder_id = ?4",
                    params![version + 1, now, source_id, folder_id]
                )?;
            }
            Ok(())
        })
    }
}
