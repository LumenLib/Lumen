use super::Database;
use log::{debug, info, warn};
use models::constructors::*;
use models::{Feed, FeedType};
use rusqlite::{OptionalExtension, Result, params};
use serde_json;

impl Database {
    // --- Feed Operations (Main DB) ---

    pub fn insert_feed(&self, feed: &Feed) -> Result<()> {
        info!("数据库: 准备写入订阅源 '{}' (ID: {})", feed.name, feed.id);
        let type_str = serde_json::to_string(&feed.feed_type)
            .unwrap_or_else(|_| "\"rss\"".to_string())
            .trim_matches('"')
            .to_string();

        self.with_conn(|conn| {
            conn.execute(
                "INSERT OR REPLACE INTO feeds (id, name, feed_type, url, last_updated_at, update_interval, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![feed.id, feed.name, type_str, feed.url, feed.last_updated_at, feed.update_interval, feed.is_dirty, feed.is_deleted, feed.version, feed.created_at, feed.updated_at],
            )?;
            Ok(())
        })
    }

    pub fn get_all_feeds(&self) -> Result<Vec<Feed>> {
        debug!("数据库: 正在获取所有订阅源列表");
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT id, name, feed_type, url, last_updated_at, update_interval, is_dirty, is_deleted, version, created_at, updated_at FROM feeds WHERE is_deleted = 0")?;
            let feed_iter = stmt.query_map([], |row| {
                let type_str: String = row.get(2)?;
                let feed_type: FeedType =
                    serde_json::from_str(&format!("\"{type_str}\"")).unwrap_or(FeedType::Rss);

                let mut feed = create_feed(
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    feed_type,
                );
                feed.url = row.get(3)?;
                feed.last_updated_at = row.get(4)?;
                feed.update_interval = row.get(5)?;
                feed.is_dirty = row.get(6)?;
                feed.is_deleted = row.get(7)?;
                feed.version = row.get(8)?;
                feed.created_at = row.get(9)?;
                feed.updated_at = row.get(10)?;
                Ok(feed)
            })?;

            let mut feeds = Vec::new();
            for f in feed_iter {
                feeds.push(f?);
            }
            debug!("数据库: 成功获取 {} 个订阅源", feeds.len());
            Ok(feeds)
        })
    }

    pub fn get_feed(&self, id: &str) -> Result<Option<Feed>> {
        debug!("数据库: 正在获取订阅源 (ID: {id})");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, feed_type, url, last_updated_at, update_interval, is_dirty, is_deleted, version, created_at, updated_at FROM feeds WHERE id = ?1 AND is_deleted = 0",
            )?;
            let mut rows = stmt.query([id])?;
            if let Some(row) = rows.next()? {
                let type_str: String = row.get(2)?;
                let feed_type: FeedType =
                    serde_json::from_str(&format!("\"{type_str}\"")).unwrap_or(FeedType::Rss);
                let mut feed = create_feed(row.get::<_, String>(0)?, row.get::<_, String>(1)?, feed_type);
                feed.url = row.get(3)?;
                feed.last_updated_at = row.get(4)?;
                feed.update_interval = row.get(5)?;
                feed.is_dirty = row.get(6)?;
                feed.is_deleted = row.get(7)?;
                feed.version = row.get(8)?;
                feed.created_at = row.get(9)?;
                feed.updated_at = row.get(10)?;
                Ok(Some(feed))
            } else {
                Ok(None)
            }
        })
    }

    pub fn update_feed(&self, feed: &Feed) -> Result<()> {
        info!(
            "数据库: 正在更新订阅源信息 '{}' (ID: {})",
            feed.name, feed.id
        );
        let type_str = serde_json::from_str::<serde_json::Value>(
            &serde_json::to_string(&feed.feed_type).unwrap_or_else(|_| "\"rss\"".to_string()),
        )
        .unwrap_or_else(|_| serde_json::json!("rss"))
        .as_str()
        .unwrap_or("rss")
        .to_string();

        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE feeds SET name = ?1, feed_type = ?2, url = ?3, last_updated_at = ?4, update_interval = ?5, is_dirty = 1, version = version + 1, updated_at = ?6 WHERE id = ?7",
                params![feed.name, type_str, feed.url, feed.last_updated_at, feed.update_interval, chrono::Local::now().timestamp(), feed.id],
            )?;
            if rows == 0 {
                warn!("数据库: 更新订阅源失败，未找到 ID 为 {} 的记录", feed.id);
            }
            Ok(())
        })
    }

    pub fn delete_feed(&self, id: &str) -> Result<()> {
        info!("数据库: 准备删除订阅源 (ID: {id})");
        self.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE feeds SET is_deleted = 1, is_dirty = 1, version = version + 1, updated_at = ?1 WHERE id = ?2",
                params![chrono::Local::now().timestamp(), id]
            )?;
            if rows > 0 {
                debug!("数据库: 订阅源 (ID: {id}) 已标记为删除");
            } else {
                warn!("数据库: 删除订阅源失败，未找到 ID 为 {id} 的记录");
            }
            Ok(())
        })
    }

    // --- 同步支持方法 ---

    pub fn get_dirty_feeds(&self) -> Result<Vec<Feed>> {
        debug!("数据库: 正在获取待同步订阅源记录");
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, feed_type, url, last_updated_at, update_interval, is_dirty, is_deleted, version, created_at, updated_at FROM feeds WHERE is_dirty = 1"
            )?;
            let iter = stmt.query_map([], |row| {
                let type_str: String = row.get(2)?;
                let feed_type: FeedType =
                    serde_json::from_str(&format!("\"{type_str}\"")).unwrap_or(FeedType::Rss);

                let mut feed = create_feed(
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    feed_type,
                );
                feed.url = row.get(3)?;
                feed.last_updated_at = row.get(4)?;
                feed.update_interval = row.get(5)?;
                feed.is_dirty = row.get(6)?;
                feed.is_deleted = row.get(7)?;
                feed.version = row.get(8)?;
                feed.created_at = row.get(9)?;
                feed.updated_at = row.get(10)?;
                Ok(feed)
            })?;
            let feeds = iter.collect::<Result<Vec<_>>>()?;
            debug!("数据库: 获取到 {} 个待同步订阅源", feeds.len());
            Ok(feeds)
        })
    }

    pub fn mark_feed_synced(&self, id: &str) -> Result<()> {
        debug!("数据库: 标记订阅源为已同步 (ID: {id})");
        self.with_conn(|conn| {
            let rows = conn.execute("UPDATE feeds SET is_dirty = 0 WHERE id = ?1", [id])?;
            if rows == 0 {
                warn!("数据库: 标记订阅源同步失败，未找到 ID 为 {id} 的记录");
            }
            Ok(())
        })
    }

    pub fn merge_remote_feed(&self, remote: Feed) -> Result<()> {
        info!(
            "数据库: 正在合并远程订阅源信息 (ID: {}, version: {})",
            remote.id, remote.version
        );
        self.with_conn(|conn| {
            let local_info: Option<(i32, bool)> = conn
                .query_row(
                    "SELECT version, is_dirty FROM feeds WHERE id = ?1",
                    [&remote.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            if let Some((local_version, is_dirty)) = local_info {
                if remote.version > local_version {
                    debug!(
                        "数据库: 远程版本较新 ({} > {})，执行覆盖更新",
                        remote.version, local_version
                    );
                    self.insert_feed_internal(conn, &remote)?;
                } else if remote.version == local_version && !is_dirty {
                    debug!("数据库: 版本一致且本地未修改，更新时间戳并标记同步");
                    conn.execute(
                        "UPDATE feeds SET updated_at = ?1, is_dirty = 0 WHERE id = ?2",
                        params![remote.updated_at, remote.id],
                    )?;
                } else {
                    debug!("数据库: 本地版本较新或有未同步修改，忽略远程更新");
                }
            } else {
                debug!("数据库: 本地未找到该订阅源，执行插入");
                self.insert_feed_internal(conn, &remote)?;
            }
            Ok(())
        })
    }

    fn insert_feed_internal(&self, conn: &rusqlite::Connection, feed: &Feed) -> Result<()> {
        let type_str = serde_json::to_string(&feed.feed_type)
            .unwrap_or_else(|_| "\"rss\"".to_string())
            .trim_matches('"')
            .to_string();

        conn.execute(
            "INSERT OR REPLACE INTO feeds (id, name, feed_type, url, last_updated_at, update_interval, is_dirty, is_deleted, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![feed.id, feed.name, type_str, feed.url, feed.last_updated_at, feed.update_interval, 0, feed.is_deleted, feed.version, feed.created_at, feed.updated_at],
        )?;
        Ok(())
    }

    pub fn purge_synced_feeds(&self) -> Result<usize> {
        info!("数据库: 正在清理已同步的删除订阅源记录");
        self.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM feeds WHERE is_deleted = 1 AND is_dirty = 0",
                [],
            )?;
            info!("数据库: 已清理 {count} 个已同步删除的订阅源");
            Ok(count)
        })
    }
}
