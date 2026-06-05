use super::Migration;
use super::utils;

/// v011 迁移
///
/// 1. 对齐 Zotero 注释颜色：Pink → Magenta, Cyan → Gray
/// 2. 为 pdf_state 表补充 auto_translate 列（v0.1.0 旧库缺少此列导致 INSERT 静默失败）
pub fn migration() -> Migration {
    Migration {
        version: "v011",
        description: "补充 pdf_state.auto_translate 列 + 注释颜色对齐 Zotero",
        up: |conn| {
            if utils::table_exists(conn, "annotations")? {
                conn.execute(
                    "UPDATE annotations SET color = 'Magenta' WHERE color = 'Pink'",
                    [],
                )?;
                conn.execute(
                    "UPDATE annotations SET color = 'Gray' WHERE color = 'Cyan'",
                    [],
                )?;
            }

            if utils::table_exists(conn, "pdf_state")? {
                utils::add_column(
                    conn,
                    "pdf_state",
                    "auto_translate",
                    "INTEGER NOT NULL DEFAULT 1",
                )?;
            }

            Ok(())
        },
    }
}
