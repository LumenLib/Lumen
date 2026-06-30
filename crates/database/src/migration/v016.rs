use super::Migration;
use super::utils;
use log::debug;

pub fn migration() -> Migration {
    Migration {
        version: "v016",
        description: "在数据库中删除 literatures.isbn, publications.issn, publications.isbn",
        up: |conn| {
            // 1. 删除 literatures.isbn
            if utils::table_exists(conn, "literatures")? {
                utils::drop_column(conn, "literatures", "isbn")?;
            }

            // 2. 删除 publications.issn 和 publications.isbn
            if utils::table_exists(conn, "publications")? {
                utils::drop_column(conn, "publications", "issn")?;
                utils::drop_column(conn, "publications", "isbn")?;
            }

            debug!("[v016] 成功删除了 isbn 和 issn 字段");
            Ok(())
        },
    }
}
