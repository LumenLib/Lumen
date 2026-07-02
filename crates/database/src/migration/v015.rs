use super::Migration;
use super::utils;
use log::{debug, error};

pub fn migration() -> Migration {
    Migration {
        version: "v015",
        description: "AI 对话支持思考过程; 删除 literatures.isbn 与 publications.isbn/issn",
        up: |conn| {
            // 1. chat_messages 添加 reasoning 列
            let table_ok = utils::table_exists(conn, "chat_messages")?;
            let col_ok = utils::column_exists(conn, "chat_messages", "reasoning")?;
            debug!(
                "[v015] table_exists(chat_messages)={table_ok}, column_exists(reasoning)={col_ok}"
            );
            if table_ok && !col_ok {
                utils::add_column(conn, "chat_messages", "reasoning", "TEXT")?;
                let verify = utils::column_exists(conn, "chat_messages", "reasoning")?;
                debug!("[v015] add_column 后验证: reasoning exists = {verify}");
                if !verify {
                    error!("[v015] FATAL: add_column 报告成功但列仍未存在！");
                }
            } else {
                debug!("[v015] 跳过: table_ok={table_ok}, column_exists={col_ok}");
            }

            // 2. 删除 literatures.isbn
            if utils::table_exists(conn, "literatures")? {
                utils::drop_column(conn, "literatures", "isbn")?;
            }

            // 3. 删除 publications.issn 和 publications.isbn
            if utils::table_exists(conn, "publications")? {
                utils::drop_column(conn, "publications", "issn")?;
                utils::drop_column(conn, "publications", "isbn")?;
            }

            debug!("[v015] 成功完成: 添加 reasoning, 删除 isbn/issn 字段");
            Ok(())
        },
    }
}
