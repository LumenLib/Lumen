use super::Migration;
use super::utils;
use log::{debug, error};

pub fn migration() -> Migration {
    Migration {
        version: "v015",
        description: "AI 对话消息支持思考过程记录",
        up: |conn| {
            let table_ok = utils::table_exists(conn, "chat_messages")?;
            let col_ok = utils::column_exists(conn, "chat_messages", "reasoning")?;
            debug!(
                "[v015] table_exists(chat_messages)={table_ok}, column_exists(reasoning)={col_ok}"
            );
            if table_ok && !col_ok {
                utils::add_column(conn, "chat_messages", "reasoning", "TEXT")?;
                // 验证列是否真的添加成功
                let verify = utils::column_exists(conn, "chat_messages", "reasoning")?;
                debug!("[v015] add_column 后验证: reasoning exists = {verify}");
                if !verify {
                    error!("[v015] FATAL: add_column 报告成功但列仍未存在！");
                }
            } else {
                debug!("[v015] 跳过: table_ok={table_ok}, column_exists={col_ok}");
            }
            Ok(())
        },
    }
}
