use super::Migration;
use super::utils;
use log::{debug, info, warn};
use rusqlite::params;

pub fn migration() -> Migration {
    Migration {
        version: "v0111",
        description: "主附件文件名添加随机后缀避免重名冲突",
        up: |conn| {
            if !utils::table_exists(conn, "attachments")? {
                return Ok(());
            }

            let mut stmt = conn.prepare(
                "SELECT id, file_name, file_path FROM attachments WHERE is_main = 1 AND is_deleted = 0",
            )?;
            let rows: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()?;

            let mut renamed_count = 0;
            for (id, file_name, file_path) in rows {
                // 跳过已有随机后缀的文件名（匹配 _xxxx.ext 模式）
                if has_random_suffix(&file_name) {
                    continue;
                }

                let new_name = insert_random_suffix(&file_name);
                let new_path = std::path::Path::new(&file_path)
                    .parent()
                    .map(|dir| dir.join(&new_name).to_string_lossy().to_string())
                    .unwrap_or_else(|| new_name.clone());

                // 物理重命名
                let old_path = std::path::Path::new(&file_path);
                if old_path.exists() {
                    let new_path_ref = std::path::Path::new(&new_path);
                    if let Err(e) = std::fs::rename(old_path, new_path_ref) {
                        warn!("迁移 v0111: 重命名文件失败 {file_name} -> {new_name}: {e}");
                        continue;
                    }
                }

                conn.execute(
                    "UPDATE attachments SET file_name = ?1, file_path = ?2, etag = NULL, hash = NULL, is_dirty = 1 WHERE id = ?3",
                    params![new_name, new_path, id],
                )?;
                renamed_count += 1;
                debug!("迁移 v0111: {file_name} -> {new_name}");
            }

            if renamed_count > 0 {
                info!("迁移 v0111: 重命名了 {renamed_count} 个主附件");
            }
            Ok(())
        },
    }
}

/// 检查文件名是否已有随机后缀（_xxxx.ext 格式，xxxx 为 4 位十六进制）
fn has_random_suffix(file_name: &str) -> bool {
    let stem = file_name.rsplit('.').nth(1).unwrap_or(file_name);
    if let Some(last_part) = stem.rsplit('_').next() {
        last_part.len() == 4 && last_part.chars().all(|c| c.is_ascii_hexdigit())
    } else {
        false
    }
}

/// 在扩展名前插入 _{4hex} 随机后缀
fn insert_random_suffix(file_name: &str) -> String {
    let suffix = &uuid::Uuid::new_v4().to_string()[..4];
    if let Some(dot_pos) = file_name.rfind('.') {
        format!(
            "{}_{suffix}{}",
            &file_name[..dot_pos],
            &file_name[dot_pos..]
        )
    } else {
        format!("{file_name}_{suffix}")
    }
}
