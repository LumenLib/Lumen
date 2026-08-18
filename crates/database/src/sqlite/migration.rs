use log::{debug, error, info};
use rusqlite::{Connection, Result};

use super::Database;

impl Database {
    pub(super) fn run_local_migrations(&self) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS _migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL DEFAULT '')",
                [],
            )?;

            if !Self::local_ts_columns_need_migration(conn)? {
                debug!("本地时间戳列已为 INTEGER，跳过迁移");
                return Ok(());
            }

            info!("本地数据库: 开始迁移时间戳列 TEXT -> INTEGER");
            conn.execute("PRAGMA foreign_keys = OFF", [])?;

            for &table in Self::TS_MIGRATION_TABLES {
                if Self::table_has_text_ts(conn, table)? {
                    Self::rebuild_table_timestamps(conn, table).map_err(|e| {
                        error!("本地迁移失败(表 {table}): {e}");
                        e
                    })?;
                }
            }

            conn.execute("PRAGMA foreign_keys = ON", [])?;

            if Self::local_ts_columns_need_migration(conn)? {
                error!("本地迁移校验失败：时间戳列仍非 INTEGER，迁移未完成");
                return Err(rusqlite::Error::ExecuteReturnedResults);
            }

            conn.execute(
                "INSERT OR REPLACE INTO _migrations (version, applied_at) VALUES (2, datetime('now'))",
                [],
            )?;
            info!("本地数据库: 时间戳列已迁移至 INTEGER (完成)");
            Ok(())
        })
    }

    /// 参与时间戳迁移的表（拥有 created_at / updated_at 列）。
    const TS_MIGRATION_TABLES: &'static [&'static str] = &[
        "literatures",
        "publications",
        "authors",
        "literature_authors",
        "folders",
        "literature_folders",
        "tags",
        "literature_tags",
        "attachments",
        "feeds",
        "feed_items",
        "literature_citations",
        "annotations",
        "literature_notes",
    ];
    fn local_ts_columns_need_migration(conn: &Connection) -> Result<bool> {
        for &table in Self::TS_MIGRATION_TABLES {
            if Self::table_has_text_ts(conn, table)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn table_has_text_ts(conn: &Connection, table: &str) -> Result<bool> {
        if !Self::table_exists(conn, table)? {
            return Ok(false);
        }
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            let ctype: String = row.get(2)?;
            Ok((name, ctype))
        })?;
        for r in rows {
            let (name, ctype) = r?;
            if (name == "created_at" || name == "updated_at") && ctype.eq_ignore_ascii_case("TEXT")
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
        let c: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?",
            [name],
            |r| r.get(0),
        )?;
        Ok(c > 0)
    }

    /// 用“建新表(INTEGER) -> 拷贝数据(CAST) -> 改名”真正改变存储类。
    /// SQLite 不支持 ALTER COLUMN 改存储类，必须重建表；同时保留二级索引。
    pub(crate) fn rebuild_table_timestamps(conn: &Connection, table: &str) -> Result<()> {
        let tmp = format!("{table}__mig_tmp");
        let has_tbl = Self::table_exists(conn, table)?;
        let has_tmp = Self::table_exists(conn, &tmp)?;
        if has_tmp {
            if has_tbl {
                // 新表已建好，仅残留临时表，清理即可
                conn.execute(&format!("DROP TABLE IF EXISTS {tmp}"), [])?;
                return Ok(());
            }
            // 仅残留临时表：旧表在“改名后建新表”前失败，还原回旧表，下次启动重试
            conn.execute(&format!("ALTER TABLE {tmp} RENAME TO {table}"), [])?;
            return Ok(());
        }

        // 捕获二级索引 DDL（自动索引 sql 为 NULL，会被排除），稍后重建
        let mut idx_defs: Vec<(String, String)> = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT name, sql FROM sqlite_master WHERE type='index' AND tbl_name=? AND sql IS NOT NULL",
            )?;
            let rows = stmt.query_map([table], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for r in rows {
                idx_defs.push(r?);
            }
        }

        // 读取原表列信息（名称 + 是否时间戳文本列）
        let mut cols: Vec<(String, bool)> = Vec::new();
        {
            let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
            let rows = stmt.query_map([], |row| {
                let name: String = row.get(1)?;
                let ctype: String = row.get(2)?;
                let is_ts = (name == "created_at" || name == "updated_at")
                    && ctype.eq_ignore_ascii_case("TEXT");
                Ok((name, is_ts))
            })?;
            for r in rows {
                cols.push(r?);
            }
        }
        if cols.is_empty() {
            return Ok(()); // 表不存在，跳过
        }

        // 取原建表语句并替换时间戳列类型为 INTEGER
        let create_sql: String = conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?",
            [table],
            |row| row.get::<_, String>(0),
        )?;
        let new_create = create_sql
            .replace("created_at TEXT", "created_at INTEGER")
            .replace("updated_at TEXT", "updated_at INTEGER");

        // 先删二级索引（避免改名后索引名冲突），再重建表
        for (name, _sql) in &idx_defs {
            conn.execute(&format!("DROP INDEX IF EXISTS {name}"), [])?;
        }
        conn.execute(&format!("ALTER TABLE {table} RENAME TO {tmp}"), [])?;
        conn.execute(&new_create, [])?;

        // 构造 INSERT ... SELECT，时间戳列做转换：
        // 数字串 -> 直接 CAST；datetime 串 -> strftime('%s')；NULL/空 -> 0
        let col_names: Vec<String> = cols.iter().map(|(n, _)| n.clone()).collect();
        let col_list = col_names.join(", ");
        let select_exprs: Vec<String> = cols
            .iter()
            .map(|(n, is_ts)| {
                if *is_ts {
                    format!(
                        "CASE WHEN {n} IS NULL OR {n} = '' THEN 0 \
                         WHEN {n} GLOB '[0-9]*' AND {n} NOT GLOB '*[^0-9]*' THEN CAST({n} AS INTEGER) \
                         ELSE CAST(strftime('%s', {n}) AS INTEGER) END"
                    )
                } else {
                    n.clone()
                }
            })
            .collect();
        let select_list = select_exprs.join(", ");
        conn.execute(
            &format!("INSERT INTO {table} ({col_list}) SELECT {select_list} FROM {tmp}"),
            [],
        )?;

        // 重建二级索引
        for (_name, sql) in &idx_defs {
            conn.execute(sql, [])?;
        }

        conn.execute(&format!("DROP TABLE {tmp}"), [])?;
        info!("本地迁移: 表 {table} 时间戳列已转为 INTEGER");
        Ok(())
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// 验证“建新表 -> 拷贝(CAST) -> 改名”能真正把 TEXT 时间戳列转为 INTEGER，
    /// 且 epoch 文本与 datetime 文本都被正确转成整数（不截断、不损坏）。
    #[test]
    fn text_timestamp_columns_convert_to_integer() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE literatures (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        // 模拟“上一版错误迁移”留下的 epoch 文本，以及一份原始 datetime 文本
        conn.execute(
            "INSERT INTO literatures VALUES ('a', '1780886744', '2026-07-21 00:00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO literatures VALUES ('b', '2026-07-21 00:00:00', '1784571065')",
            [],
        )
        .unwrap();

        // 迁移前：列仍是 TEXT
        assert!(Database::table_has_text_ts(&conn, "literatures").unwrap());

        // 执行重建式迁移
        Database::rebuild_table_timestamps(&conn, "literatures").unwrap();

        // 迁移后：列类型必须为 INTEGER
        let types: Vec<(String, String)> = {
            let mut stmt = conn.prepare("PRAGMA table_info(literatures)").unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, String>(2)?)))
                .unwrap();
            rows.map(|x| x.unwrap()).collect()
        };
        for (name, t) in &types {
            if name == "created_at" || name == "updated_at" {
                assert_eq!(t.to_uppercase(), "INTEGER", "列 {name} 应为 INTEGER");
            }
        }

        // 值应为整数且转换正确
        let expected_dated: i64 = conn
            .query_row(
                "SELECT CAST(strftime('%s','2026-07-21 00:00:00') AS INTEGER)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let vals: Vec<(i64, i64)> = {
            let mut stmt = conn
                .prepare("SELECT created_at, updated_at FROM literatures ORDER BY id")
                .unwrap();
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
                .unwrap();
            rows.map(|x| x.unwrap()).collect()
        };
        assert_eq!(vals.len(), 2);
        assert_eq!(vals[0].0, 1780886744); // epoch 文本 -> 整数
        assert_eq!(vals[0].1, expected_dated); // datetime 文本 -> strftime 整数
        assert_eq!(vals[1].0, expected_dated);
        assert_eq!(vals[1].1, 1784571065);

        // 数据完整性：行数不变
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM literatures", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 2);
    }
}
