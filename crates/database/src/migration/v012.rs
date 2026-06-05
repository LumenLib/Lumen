use super::Migration;
use super::utils;
use log::info;

/// v012 迁移
///
/// 1. 创建 literature_notes 表（1:N 多笔记卡片）
/// 2. 将 literatures.notes 数据迁移到 literature_notes
/// 3. 建索引
pub fn migration() -> Migration {
    Migration {
        version: "v012",
        description: "文献笔记拆分为独立表 (1:N 多笔记卡片)",
        up: |conn| {
            if utils::table_exists(conn, "literature_notes")? {
                let cols = utils::get_column_names(conn, "literature_notes")?;
                if cols.len() == 6 {
                    conn.execute("DROP TABLE literature_notes", [])?;
                    info!("迁移: 已删除旧的 literature_notes 表 (1:1 schema)");
                } else {
                    let has_id = cols.iter().any(|c| c == "id");
                    if has_id {
                        info!("迁移: literature_notes 已为 1:N schema，跳过");
                        return Ok(());
                    }
                }
            }

            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS literature_notes (
                    id              TEXT PRIMARY KEY,
                    literature_id   TEXT NOT NULL,
                    title           TEXT NOT NULL DEFAULT '',
                    content         TEXT NOT NULL DEFAULT '',
                    sort_order      INTEGER NOT NULL DEFAULT 0,
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL
                );

                CREATE INDEX IF NOT EXISTS idx_literature_notes_lit_id
                    ON literature_notes(literature_id, sort_order);",
            )?;

            if utils::table_exists(conn, "literatures")?
                && utils::column_exists(conn, "literatures", "notes")?
            {
                let now = chrono::Utc::now().timestamp();
                let rows = conn.execute(
                    "INSERT INTO literature_notes (id, literature_id, title, content, sort_order, created_at, updated_at)
                     SELECT
                        lower(hex(randomblob(16))),
                        id,
                        '笔记 1',
                        COALESCE(notes, ''),
                        0,
                        ?1,
                        ?1
                     FROM literatures
                     WHERE notes IS NOT NULL AND notes != ''",
                    [now],
                )?;
                if rows > 0 {
                    info!("迁移: 已将 {rows} 条文献笔记迁移至新的 literature_notes 表");
                }
            }

            Ok(())
        },
    }
}
