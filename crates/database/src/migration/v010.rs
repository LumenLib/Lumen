use super::Migration;
use super::utils;
/// v010 迁移
///
/// 对应软件版本 0.1.0，包含以下变更：
/// - `lumen.db.annotations`: 添加 `is_dirty` 和 `version` 列（修复旧版缺列问题）
/// - `state.db.pdf_state`: 添加 `auto_translate` 列
pub fn migration() -> Migration {
    Migration {
        version: "v010",
        description: "修复 annotations 缺列 + 添加 pdf_state.auto_translate",
        up: |conn| {
            let tables = ["annotations", "pdf_state"];
            for table in tables {
                if !utils::table_exists(conn, table)? {
                    continue;
                }
                match table {
                    "annotations" => {
                        utils::add_column(conn, "annotations", "is_dirty", "INTEGER DEFAULT 0")?;
                        if !utils::column_exists(conn, "annotations", "version")? {
                            utils::add_column(conn, "annotations", "version", "INTEGER DEFAULT 1")?;
                        }
                    }
                    "pdf_state" => {
                        utils::add_column(conn, "pdf_state", "auto_translate", "INTEGER NOT NULL DEFAULT 1")?;
                    }
                    _ => {}
                }
            }
            Ok(())
        },
    }
}
