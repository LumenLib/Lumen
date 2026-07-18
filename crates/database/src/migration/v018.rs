use super::Migration;
use super::utils;
use log::debug;

pub fn migration() -> Migration {
    Migration {
        version: "v018",
        description: "literature_notes 添加软删除字段 (is_deleted, is_dirty, version, updated_at)",
        up: |conn| {
            if utils::table_exists(conn, "literature_notes")? {
                utils::add_column(conn, "literature_notes", "is_deleted", "INTEGER DEFAULT 0")?;
                utils::add_column(conn, "literature_notes", "is_dirty", "INTEGER DEFAULT 0")?;
                utils::add_column(conn, "literature_notes", "version", "INTEGER DEFAULT 1")?;
                utils::add_column(conn, "literature_notes", "updated_at", "INTEGER DEFAULT 0")?;
            }
            debug!("[v018] 完成: literature_notes 添加软删除字段");
            Ok(())
        },
    }
}
