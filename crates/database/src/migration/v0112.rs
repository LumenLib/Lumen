use super::Migration;
use chrono::NaiveDateTime;
use log::{debug, info, warn};
use rusqlite::params;

pub fn migration() -> Migration {
    Migration {
        version: "v0112",
        description: "修复 annotations 中 TEXT 格式的时间戳，转为 INTEGER (unix 秒)",
        up: |conn| {
            if !super::utils::table_exists(conn, "annotations")? {
                return Ok(());
            }
            let fixed = fix_timestamp_columns(conn, "annotations")?;
            if fixed > 0 {
                info!("[v0112] 修复了 annotations 表 {fixed} 个 TEXT 时间戳");
            }
            debug!("[v0112] 完成: annotations 时间戳类型修复");
            Ok(())
        },
    }
}

/// 将指定表中 created_at / updated_at 列里 TEXT 格式的时间值解析为 unix 秒（INTEGER）
fn fix_timestamp_columns(conn: &rusqlite::Connection, table: &str) -> rusqlite::Result<usize> {
    let mut total = 0;
    for col in ["created_at", "updated_at"] {
        let query = format!(
            "SELECT id, {col} FROM {table} WHERE typeof({col}) != 'integer' AND typeof({col}) != 'real'"
        );
        let mut stmt = conn.prepare(&query)?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        for (id, text_val) in rows {
            let ts = parse_timestamp(&text_val);
            let update = format!("UPDATE {table} SET {col} = ?1 WHERE id = ?2");
            conn.execute(&update, params![ts, id])?;
            total += 1;
        }
    }
    Ok(total)
}

/// 把文本时间值解析为 unix 秒。支持 "YYYY-MM-DD HH:MM:SS" 与纯数字字符串。
fn parse_timestamp(s: &str) -> i64 {
    let s = s.trim();
    // 已经是数字字符串
    if let Ok(n) = s.parse::<i64>() {
        return n;
    }
    // "YYYY-MM-DD HH:MM:SS"（按本地时区解释）
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        if let Some(local) = ndt.and_local_timezone(chrono::Local).single() {
            return local.timestamp();
        }
        return ndt.and_utc().timestamp();
    }
    warn!("[v0112] 无法解析时间戳 '{s}'，置为 0");
    0
}
