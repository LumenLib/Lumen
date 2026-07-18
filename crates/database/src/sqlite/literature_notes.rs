use super::Database;
use log::debug;
use models::LiteratureNote;
use rusqlite::{Result, params};
use uuid::Uuid;

impl Database {
    /// 列出某文献的所有笔记（按 sort_order 升序）
    pub fn list_notes(&self, literature_id: &str) -> Result<Vec<LiteratureNote>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, literature_id, title, content, sort_order, created_at, updated_at
                 FROM literature_notes
                 WHERE literature_id = ?1 AND is_deleted = 0
                 ORDER BY sort_order ASC, created_at ASC",
            )?;

            let rows = stmt.query_map([literature_id], |row| {
                Ok(LiteratureNote {
                    id: row.get(0)?,
                    literature_id: row.get(1)?,
                    title: row.get(2)?,
                    content: row.get(3)?,
                    sort_order: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;

            let mut notes = Vec::new();
            for row in rows {
                notes.push(row?);
            }
            Ok(notes)
        })
    }

    /// 新建笔记
    ///
    /// 返回新笔记的 ID
    pub fn create_note(&self, literature_id: &str, title: &str) -> Result<String> {
        debug!("数据库: 新建笔记 (literature_id={literature_id})");
        let now = chrono::Utc::now().timestamp();
        let id = Uuid::new_v4().to_string();
        self.with_transaction(|tx| {
            let next_order: i32 = tx
                .query_row(
                    "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM literature_notes WHERE literature_id = ?1",
                    [literature_id],
                    |row| row.get(0),
                )?;

            tx.execute(
                "INSERT INTO literature_notes
                    (id, literature_id, title, content, sort_order, created_at, updated_at, is_dirty)
                 VALUES (?1, ?2, ?3, '', ?4, ?5, ?5, 1)",
                params![id, literature_id, title, next_order, now],
            )?;

            bump_literature_version_in_tx(tx, literature_id, now)?;
            Ok(())
        })?;
        Ok(id)
    }

    /// 更新笔记的标题和/或内容
    pub fn update_note(
        &self,
        note_id: &str,
        title: Option<&str>,
        content: Option<&str>,
    ) -> Result<bool> {
        debug!("数据库: 更新笔记 (id={note_id})");
        let now = chrono::Utc::now().timestamp();
        self.with_transaction(|tx| {
            let rows = tx.execute(
                "UPDATE literature_notes
                 SET title = COALESCE(?2, title),
                     content = COALESCE(?3, content),
                     updated_at = ?4,
                     is_dirty = 1
                 WHERE id = ?1",
                params![note_id, title, content, now],
            )?;
            if rows == 0 {
                return Ok(false);
            }

            let lit_id: String = tx.query_row(
                "SELECT literature_id FROM literature_notes WHERE id = ?1",
                [note_id],
                |row| row.get(0),
            )?;

            bump_literature_version_in_tx(tx, &lit_id, now)?;
            Ok(true)
        })
    }

    /// 删除笔记
    pub fn delete_note(&self, note_id: &str) -> Result<bool> {
        debug!("数据库: 删除笔记 (id={note_id})");
        let now = chrono::Utc::now().timestamp();
        self.with_transaction(|tx| {
            let lit_id: Option<String> = tx
                .query_row(
                    "SELECT literature_id FROM literature_notes WHERE id = ?1",
                    [note_id],
                    |row| row.get(0),
                )
                .ok();

            let rows = tx.execute(
                "UPDATE literature_notes SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE id = ?2",
                params![now, note_id],
            )?;
            if rows == 0 {
                return Ok(false);
            }

            if let Some(lit_id) = lit_id {
                bump_literature_version_in_tx(tx, &lit_id, now)?;
            }
            Ok(true)
        })
    }

    /// 仅触发 literatures.version+1 通知 DataStore 刷新
    pub fn bump_literature_version(&self, literature_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE literatures SET version = version + 1, updated_at = ?1, is_dirty = 1
                 WHERE id = ?2",
                params![now, literature_id],
            )?;
            Ok(())
        })
    }
}

/// 在事务内触发 literatures.version+1
fn bump_literature_version_in_tx(
    tx: &rusqlite::Transaction<'_>,
    literature_id: &str,
    now: i64,
) -> Result<()> {
    tx.execute(
        "UPDATE literatures SET version = version + 1, updated_at = ?1, is_dirty = 1
         WHERE id = ?2",
        params![now, literature_id],
    )?;
    Ok(())
}
