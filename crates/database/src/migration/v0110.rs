use super::Migration;
use super::utils;
use log::debug;

pub fn migration() -> Migration {
    Migration {
        version: "v0110",
        description: "attachments 添加 hash 列 (客户端内容指纹)",
        up: |conn| {
            if utils::table_exists(conn, "attachments")? {
                utils::add_column(conn, "attachments", "hash", "TEXT")?;
            }
            debug!("[v0110] 完成: attachments 添加 hash 列");
            Ok(())
        },
    }
}
