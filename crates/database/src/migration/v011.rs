use super::Migration;
use super::utils;

/// v011 迁移
///
/// 对应软件版本 0.1.1，将注释颜色枚举重命名为 Zotero 标准色板：
/// - Pink → Magenta
/// - Cyan → Gray
pub fn migration() -> Migration {
    Migration {
        version: "v011",
        description: "对齐 Zotero 注释颜色 + 重命名 Pink/Cyan 为 Magenta/Gray",
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
            Ok(())
        },
    }
}
