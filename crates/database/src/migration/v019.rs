use super::Migration;
use super::utils;
use log::debug;

pub fn migration() -> Migration {
    Migration {
        version: "v019",
        description: "literatures 删除废弃的 keywords 和 notes 列",
        up: |conn| {
            if utils::table_exists(conn, "literatures")? {
                utils::drop_column(conn, "literatures", "keywords")?;
                utils::drop_column(conn, "literatures", "notes")?;
            }
            debug!("[v019] 完成: literatures 删除 keywords 和 notes 列");
            Ok(())
        },
    }
}
