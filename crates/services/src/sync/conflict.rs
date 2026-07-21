//! 同步冲突决策层 (`services::sync::conflict`)
//!
//! 数据库 `crate::Database` 只暴露**盲写原子原语**（`apply_remote_*`）、
//! **清脏时间戳原语**（`mark_*_up_to_date`）与**状态读取原语**
//! （`get_*_sync_state`）——它不做任何业务决策、不 warn、不返回冲突。
//!
//! 本模块集中所有 13 类记录的**合并决策**（覆盖 / 保留本地 / 标记同步 /
//! 冲突上抛），其分支逻辑与迁移前 `database` 内的 `merge_remote_*` 逐一对应，
//! 保证行为完全一致：
//! - 文献：`version>local && !dirty` → 覆盖；`== && !dirty` → 标记同步；
//!   其余（含 `> && dirty`）→ 上抛 `Conflict`。
//! - 附件 / 标签 / 关联：`> && !dirty` → 覆盖；`> && dirty` → 保留本地（warn，
//!   不进冲突）；`== && !dirty` → 标记同步；其余忽略。
//! - 引用（dirty-aware）：本地脏时仅 `remote>local` 覆盖，否则保留；本地干净时
//!   `remote>local` 覆盖，否则忽略；无本地 → 插入。
//! - 注释：`> && !dirty` → 覆盖（INSERT OR REPLACE）；`> && dirty` → 保留本地；
//!   `== && !dirty` → 标记同步；其余忽略；无本地 → 覆盖插入。
//! - 笔记：`> && !dirty` → 覆盖（UPDATE）；`> && dirty` → 保留本地；
//!   `== && !dirty` → 标记同步；无本地 → 插入。
//! - 作者 / 出版源 / 文件夹 / 订阅源 / 订阅条目：`> ` → 覆盖（不查 dirty）；
//!   `== && !dirty` → 标记同步；其余忽略；无本地 → 插入。

use anyhow::Result;
use database::Database;
use log::{debug, warn};
use models::{
    Annotation, Attachment, Author, Citation, Feed, FeedItem, Folder, Literature, LiteratureNote,
    Publication, Tag,
};

pub fn merge_remote_author(db: &Database, remote: Author) -> Result<()> {
    match db.get_author_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version {
                debug!(
                    "Sync: 远程作者版本较新 ({} > {})，覆盖本地",
                    remote.version, local_version
                );
                db.apply_remote_author(&remote)?;
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 作者版本一致且本地未修改，标记同步");
                db.mark_author_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该作者，插入远程记录");
            db.apply_remote_author(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_folder(db: &Database, remote: Folder) -> Result<()> {
    match db.get_folder_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version {
                debug!(
                    "Sync: 远程文件夹版本较新 ({} > {})，覆盖本地",
                    remote.version, local_version
                );
                db.apply_remote_folder(&remote)?;
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 文件夹版本一致且本地未修改，标记同步");
                db.mark_folder_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该文件夹，插入远程记录");
            db.apply_remote_folder(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_publication(db: &Database, remote: Publication) -> Result<()> {
    match db.get_publication_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version {
                debug!(
                    "Sync: 远程出版源版本较新 ({} > {})，覆盖本地",
                    remote.version, local_version
                );
                db.apply_remote_publication(&remote)?;
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 出版源版本一致且本地未修改，标记同步");
                db.mark_publication_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该出版源，插入远程记录");
            db.apply_remote_publication(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_feed(db: &Database, remote: Feed) -> Result<()> {
    match db.get_feed_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version {
                debug!(
                    "Sync: 远程订阅源版本较新 ({} > {})，覆盖本地",
                    remote.version, local_version
                );
                db.apply_remote_feed(&remote)?;
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 订阅源版本一致且本地未修改，标记同步");
                db.mark_feed_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该订阅源，插入远程记录");
            db.apply_remote_feed(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_feed_item(db: &Database, remote: FeedItem) -> Result<()> {
    match db.get_feed_item_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version {
                debug!(
                    "Sync: 远程订阅条目版本较新 ({} > {})，覆盖本地",
                    remote.version, local_version
                );
                db.apply_remote_feed_item(&remote)?;
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 订阅条目版本一致且本地未修改，标记同步");
                db.mark_feed_item_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该订阅条目，插入远程记录");
            db.apply_remote_feed_item(&remote)?;
        }
    }
    Ok(())
}

/// 文献合并：返回 `Some(remote)` 表示冲突，由上层 `SyncStatus::Conflict` 上抛。
pub fn merge_remote_literature(db: &Database, remote: Literature) -> Result<Option<Literature>> {
    match db.get_literature_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version && !is_dirty {
                debug!(
                    "Sync: 远程文献版本较新且本地无修改 ({} -> {})，覆盖",
                    local_version, remote.version
                );
                db.apply_remote_literature(&remote)?;
                Ok(None)
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 文献版本一致且本地未修改，仅更新时间戳");
                db.mark_literature_up_to_date(&remote)?;
                Ok(None)
            } else {
                warn!(
                    "Sync: 发现文献合并冲突 (ID: {}) 本地版本: {}, 远程版本: {}, 本地Dirty: {}",
                    remote.id, local_version, remote.version, is_dirty
                );
                Ok(Some(remote))
            }
        }
        None => {
            debug!("Sync: 本地无该文献，插入远程记录");
            db.apply_remote_literature(&remote)?;
            Ok(None)
        }
    }
}

pub fn merge_remote_attachment(db: &Database, remote: Attachment) -> Result<()> {
    match db.get_attachment_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version && !is_dirty {
                debug!(
                    "Sync: 远程附件版本较新 ({} > {}) 且本地无修改，覆盖",
                    remote.version, local_version
                );
                db.apply_remote_attachment(&remote)?;
            } else if remote.version > local_version && is_dirty {
                warn!(
                    "Sync: 附件合并冲突 (ID: {}) 远程版本: {}, 本地版本: {}, 本地Dirty: true. 保留本地修改。",
                    remote.id, remote.version, local_version
                );
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 附件版本一致且本地未修改，标记同步");
                db.mark_attachment_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该附件，插入远程记录");
            db.apply_remote_attachment(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_tag(db: &Database, remote: Tag) -> Result<()> {
    match db.get_tag_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version && !is_dirty {
                debug!(
                    "Sync: 远程标签版本较新 ({} > {})，覆盖",
                    remote.version, local_version
                );
                db.apply_remote_tag(&remote)?;
            } else if remote.version > local_version && is_dirty {
                warn!(
                    "Sync: 标签合并冲突 (ID: {}) 远程版本: {}, 本地版本: {}, 本地Dirty: true. 保留本地修改。",
                    remote.id, remote.version, local_version
                );
            } else if remote.version == local_version && !is_dirty {
                debug!("Sync: 标签版本一致且本地未修改，标记同步");
                db.mark_tag_up_to_date(&remote)?;
            }
        }
        None => {
            debug!("Sync: 本地无该标签，插入远程记录");
            db.apply_remote_tag(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_citation(db: &Database, remote: Citation) -> Result<()> {
    match db.get_citation_sync_state(&remote.source_id, &remote.target_id)? {
        Some((local_version, local_dirty)) => {
            if local_dirty {
                if remote.version > local_version {
                    debug!(
                        "Sync: Remote citation version {} > local {}, overwriting.",
                        remote.version, local_version
                    );
                    db.apply_remote_citation(&remote)?;
                } else {
                    debug!("Sync: Local citation dirty and version >= remote, keeping local.");
                }
            } else if remote.version > local_version {
                db.apply_remote_citation(&remote)?;
            }
        }
        None => {
            db.apply_remote_citation(&remote)?;
        }
    }
    Ok(())
}

pub fn merge_remote_annotation(db: &Database, ann: Annotation) -> Result<()> {
    let mut do_apply = false;
    match db.get_annotation_sync_state(&ann.id)? {
        Some((local_version, is_dirty)) => {
            if ann.version > local_version && !is_dirty {
                do_apply = true;
            } else if ann.version > local_version && is_dirty {
                warn!(
                    "Sync: 注释合并冲突 (ID: {}) 远程版本: {}, 本地版本: {}, 本地Dirty: true. 保留本地修改。",
                    ann.id, ann.version, local_version
                );
            } else if ann.version == local_version && !is_dirty {
                debug!("Sync: 注释版本一致且本地未修改，标记同步");
                db.mark_annotation_up_to_date(&ann)?;
            }
        }
        None => {
            do_apply = true;
        }
    }
    if do_apply {
        db.apply_remote_annotation(&ann)?;
    }
    Ok(())
}

pub fn merge_remote_note(db: &Database, remote: LiteratureNote) -> Result<()> {
    match db.get_note_sync_state(&remote.id)? {
        Some((local_version, is_dirty)) => {
            if remote.version > local_version && !is_dirty {
                db.apply_remote_note(&remote)?;
            } else if remote.version > local_version && is_dirty {
                warn!(
                    "Sync: 笔记合并冲突 (ID: {}) 远程版本: {}, 本地版本: {}, 本地Dirty: true. 保留本地修改。",
                    remote.id, remote.version, local_version
                );
            } else if remote.version == local_version && !is_dirty {
                db.mark_note_up_to_date(&remote)?;
            }
        }
        None => {
            db.apply_remote_note(&remote)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn merge_remote_relation(
    db: &Database,
    table: &str,
    lit_id: &str,
    target_id: &str,
    sort_order: Option<i32>,
    is_deleted: bool,
    version: i32,
) -> Result<()> {
    match db.get_relation_sync_state(table, lit_id, target_id)? {
        Some((local_v, is_dirty)) => {
            if version > local_v && !is_dirty {
                debug!(
                    "Sync: 远程关联版本较新且本地无修改 (Table: {table}, {local_v} -> {version})，覆盖"
                );
                db.apply_remote_relation(table, lit_id, target_id, sort_order, is_deleted, version)?;
            } else if version == local_v && !is_dirty {
                debug!("Sync: 关联版本一致且本地无修改，标记同步");
                db.mark_relation_up_to_date(table, lit_id, target_id)?;
            }
        }
        None => {
            debug!("Sync: 本地无该关联，插入远程记录 (Table: {table})");
            db.apply_remote_relation(table, lit_id, target_id, sort_order, is_deleted, version)?;
        }
    }
    Ok(())
}
