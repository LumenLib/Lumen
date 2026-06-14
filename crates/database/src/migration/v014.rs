use super::Migration;
use super::utils;
use log::{debug, error};

pub fn migration() -> Migration {
    Migration {
        version: "v014",
        description: "AI 对话上下文摘要持久化",
        up: |conn| {
            let table_ok = utils::table_exists(conn, "chat_sessions")?;
            let col_ok = utils::column_exists(conn, "chat_sessions", "compressed_summary")?;
            debug!(
                "[v014] table_exists(chat_sessions)={table_ok}, column_exists(compressed_summary)={col_ok}"
            );
            if table_ok && !col_ok {
                utils::add_column(
                    conn,
                    "chat_sessions",
                    "compressed_summary",
                    "TEXT NOT NULL DEFAULT ''",
                )?;
                // 验证列是否真的添加成功
                let verify = utils::column_exists(conn, "chat_sessions", "compressed_summary")?;
                debug!("[v014] add_column 后验证: compressed_summary exists = {verify}");
                if !verify {
                    error!("[v014] FATAL: add_column 报告成功但列仍未存在！");
                }
            } else {
                debug!("[v014] 跳过: table_ok={table_ok}, column_exists={col_ok}");
            }
            Ok(())
        },
    }
}
