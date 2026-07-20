use super::MySqlManager;
use super::rows::{
    AttachmentRow, AuthorRow, CitationRow, FeedItemRow, FeedRow, FolderRow, LiteratureRow,
    PublicationRow, TagRow,
};
use super::schema::ensure_remote_tables;
use crate::Database;

use anyhow::{Result, anyhow};
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use log::{debug, error, info};
use models::{Literature, Tag};
use mysql_async::{params, prelude::*};
use std::sync::Arc;

fn author_full_name(a: &models::Author) -> String {
    if let Some(ref middle) = a.middle_name {
        format!("{} {} {}", a.first_name, middle, a.last_name)
    } else {
        format!("{} {}", a.first_name, a.last_name)
    }
}

pub async fn sync_metadata(
    manager: &MySqlManager,
    db: Arc<Database>,
    base_path: &std::path::Path,
    allowed_attachment_ids: Option<&[String]>,
) -> Result<Vec<Literature>> {
    let (use_remote, host) = {
        let c = manager.config.read().unwrap();
        (c.use_remote, c.host.clone())
    };
    if !use_remote {
        return Ok(Vec::new());
    }
    info!("MySQL: 开始元数据同步 (远程主机: {host})");

    let base_path_buf = base_path.to_path_buf();
    let allowed_ids = allowed_attachment_ids.map(<[std::string::String]>::to_vec);

    let pool = manager.get_pool().await?;
    let db_clone = db.clone();
    let manager_config = manager.get_config();

    let sync_task = async move {
        info!("MySQL: 获取连接池成功");
        let mut conn = pool.get_conn().await?;
        info!("MySQL: 建立数据库连接成功");

        ensure_remote_tables(&mut conn).await?;

        info!("MySQL: 正在同步标签...");
        if let Err(e) = perform_sync_tags(
            manager_config.use_remote,
            &manager_config.host,
            &mut conn,
            db_clone.clone(),
        )
        .await
        {
            error!("MySQL: 标签同步失败: {e}");
        }

        info!("MySQL: 正在推送本地变更...");
        push_dirty_records(
            &mut conn,
            db_clone.clone(),
            &base_path_buf,
            allowed_ids.as_deref(),
        )
        .await?;

        info!("MySQL: 正在拉取远程全部数据...");
        let conflicts = pull_remote_changes(&mut conn, db_clone.clone(), &base_path_buf).await?;

        info!(
            "MySQL: 元数据同步任务圆满完成，发现 {} 个冲突",
            conflicts.len()
        );
        Ok(conflicts)
    };

    if let Ok(result) = tokio::time::timeout(std::time::Duration::from_secs(30), sync_task).await {
        result
    } else {
        let msg = "MySQL 同步超时 (30秒)";
        error!("{msg}");
        Err(anyhow!(msg))
    }
}

pub async fn sync_tags(
    manager: &MySqlManager,
    conn: &mut mysql_async::Conn,
    db: Arc<Database>,
) -> Result<Vec<Tag>> {
    let (use_remote, host) = {
        let c = manager.config.read().unwrap();
        (c.use_remote, c.host.clone())
    };
    perform_sync_tags(use_remote, &host, conn, db).await
}

async fn perform_sync_tags(
    use_remote: bool,
    host: &str,
    conn: &mut mysql_async::Conn,
    db: Arc<Database>,
) -> Result<Vec<Tag>> {
    if !use_remote {
        return Ok(Vec::new());
    }
    info!("MySQL: 开始标签同步 (远程主机: {host})");

    ensure_remote_tables(conn).await?;

    let dirty_tags = db.get_dirty_tags()?;
    if !dirty_tags.is_empty() {
        info!("MySQL: 正在推送 {} 个本地变更标签...", dirty_tags.len());
        for tag in dirty_tags {
            let stmt = "INSERT INTO tags (id, name, color, is_deleted, version, created_at, updated_at) VALUES (:id, :name, :color, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE name=VALUES(name), color=VALUES(color), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            let params = params! {
                "id" => &tag.id,
                "name" => &tag.name,
                "color" => &tag.color,
                "is_deleted" => tag.is_deleted,
                "version" => tag.version,
                "created_at" => &tag.created_at,
            };
            if let Err(e) = conn.exec_drop(stmt, params).await {
                error!(
                    "MySQL: 推送标签失败 [名称: {}, ID: {}]: {}",
                    tag.name, tag.id, e
                );
            } else {
                debug!("MySQL: 成功推送标签 [名称: {}, ID: {}]", tag.name, tag.id);
                if let Err(e) = db.mark_tag_clean(&tag.id) {
                    error!("MySQL: 更新本地标签同步状态失败 (ID: {}): {}", tag.id, e);
                }
            }
        }
    }

    let last_sync_time = db
        .get_last_sync_time("tags")?
        .unwrap_or_else(|| "0".to_string());

    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, name, color, is_deleted, version, created_at, updated_at FROM tags WHERE updated_at > :t", params! { "t" => &last_sync_time }).await?;

    let mut updated_tags = Vec::new();
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 个远程标签更新", rows.len());
        let mut max_ua: i64 = last_sync_time.parse().unwrap_or(0);

        for row in rows {
            let r = TagRow::from_mysql_row(row)?;
            if let Some(ua) = r.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            let tag = r.into_model();
            db.merge_remote_tag(tag.clone())?;
            updated_tags.push(tag);
        }
        db.set_last_sync_time("tags", &max_ua.to_string())?;
    }

    Ok(updated_tags)
}

async fn push_dirty_records(
    conn: &mut mysql_async::Conn,
    db: Arc<Database>,
    base_path: &std::path::Path,
    allowed_attachment_ids: Option<&[String]>,
) -> Result<()> {
    let authors = db.get_dirty_authors()?;
    if !authors.is_empty() {
        info!("MySQL: 正在推送 {} 个脏作者记录...", authors.len());
        for a in authors {
            debug!("MySQL: 推送作者: {} (ID: {})", author_full_name(&a), a.id);
            let q = "INSERT INTO authors (id, first_name, last_name, middle_name, is_deleted, version, created_at, updated_at) VALUES (:id, :first_name, :last_name, :middle_name, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE first_name=VALUES(first_name), last_name=VALUES(last_name), middle_name=VALUES(middle_name), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &a.id, "first_name" => &a.first_name, "last_name" => &a.last_name, "middle_name" => &a.middle_name, "is_deleted" => a.is_deleted, "version" => a.version, "created_at" => &a.created_at }).await {
                error!("MySQL: 推送作者失败 '{}' (ID: {}): {}", author_full_name(&a), a.id, e);
            } else if let Err(e) = db.mark_author_synced(&a.id) {
                error!("MySQL: 更新本地作者同步状态失败 (ID: {}): {}", a.id, e);
            }
        }
    }

    let folders = db.get_dirty_folders()?;
    if !folders.is_empty() {
        info!("MySQL: 正在推送 {} 个脏文件夹记录...", folders.len());
        for f in folders {
            debug!("MySQL: 推送文件夹: {} (ID: {})", f.name, f.id);
            let q = "INSERT INTO folders (id, name, folder_type, parent_id, is_deleted, version, created_at, updated_at) VALUES (:id, :name, :type, :parent_id, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE name=VALUES(name), folder_type=VALUES(folder_type), parent_id=VALUES(parent_id), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &f.id, "name" => &f.name, "type" => serde_json::to_string(&f.folder_type).unwrap_or_default().replace('"', ""), "parent_id" => &f.parent_id, "is_deleted" => f.is_deleted, "version" => f.version, "created_at" => &f.created_at }).await {
                error!("MySQL: 推送文件夹失败 '{}' (ID: {}): {}", f.name, f.id, e);
            } else if let Err(e) = db.mark_folder_synced(&f.id) {
                error!("MySQL: 更新本地文件夹同步状态失败 (ID: {}): {}", f.id, e);
            }
        }
    }

    let pubs = db.get_dirty_publications()?;
    if !pubs.is_empty() {
        info!("MySQL: 正在推送 {} 个脏出版源记录...", pubs.len());
        for p in pubs {
            debug!("MySQL: 推送出版源: {} (ID: {})", p.name, p.id);
            let q = "INSERT INTO publications (id, name, publication_type, abbreviation, publisher, ccf_rank, jcr_rank, cas_rank, is_deleted, version, created_at, updated_at) VALUES (:id, :name, :type, :abbr, :pub, :ccf, :jcr, :cas, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE name=VALUES(name), publication_type=VALUES(publication_type), abbreviation=VALUES(abbreviation), publisher=VALUES(publisher), ccf_rank=VALUES(ccf_rank), jcr_rank=VALUES(jcr_rank), cas_rank=VALUES(cas_rank), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn
                .exec_drop(
                    q,
                    params! {
                        "id" => &p.id,
                        "name" => &p.name,
                        "type" => p.publication_type.to_string(),
                        "abbr" => &p.abbreviation,
                        "pub" => &p.publisher,
                        "ccf" => &p.ccf_rank,
                        "jcr" => &p.jcr_rank,
                        "cas" => &p.cas_rank,
                        "is_deleted" => p.is_deleted,
                        "version" => p.version,
                        "created_at" => &p.created_at
                    },
                )
                .await
            {
                error!("MySQL: 推送出版源失败 '{}' (ID: {}): {}", p.name, p.id, e);
            } else if let Err(e) = db.mark_publication_synced(&p.id) {
                error!("MySQL: 更新本地出版源同步状态失败 (ID: {}): {}", p.id, e);
            }
        }
    }

    let lits = db.get_dirty_literatures()?;
    if !lits.is_empty() {
        info!("MySQL: 正在推送 {} 篇文献修改...", lits.len());
        for lit in lits {
            debug!("MySQL: 推送文献: '{}' (ID: {})", lit.title, lit.id);
            let publication_id = lit.publication.as_ref().map(|p| p.id.clone());
            let q = "INSERT INTO literatures (id, title, year, month, day, type, publication_id, volume, issue, pages, abstract_text, doi, arxiv_id, url, rating, reading_status, is_deleted, version, created_at, updated_at) VALUES (:id, :title, :year, :month, :day, :type, :pub_id, :volume, :issue, :pages, :abstract_text, :doi, :arxiv_id, :url, :rating, :reading_status, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE title=VALUES(title), year=VALUES(year), month=VALUES(month), day=VALUES(day), type=VALUES(type), publication_id=VALUES(publication_id), volume=VALUES(volume), issue=VALUES(issue), pages=VALUES(pages), abstract_text=VALUES(abstract_text), doi=VALUES(doi), arxiv_id=VALUES(arxiv_id), url=VALUES(url), rating=VALUES(rating), reading_status=VALUES(reading_status), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &lit.id, "title" => &lit.title, "year" => lit.year, "month" => lit.month, "day" => lit.day, "type" => serde_json::to_string(&lit.literature_type).unwrap_or_default().replace('"', ""), "pub_id" => &publication_id, "volume" => &lit.volume, "issue" => &lit.issue, "pages" => &lit.pages, "abstract_text" => &lit.abstract_text, "doi" => &lit.doi, "arxiv_id" => &lit.arxiv_id, "url" => &lit.url, "rating" => lit.rating, "reading_status" => lit.reading_status.to_string(), "is_deleted" => lit.is_deleted, "version" => lit.version, "created_at" => &lit.created_at }).await {
                error!("MySQL: 推送文献失败 '{}' (ID: {}): {}", lit.title, lit.id, e);
            } else if let Err(e) = db.mark_literature_synced(&lit.id) {
                error!("MySQL: 更新本地文献同步状态失败 (ID: {}): {}", lit.id, e);
            }
        }
    }

    let (auth_rels, fold_rels, tag_rels) = db.get_dirty_relations()?;
    if !auth_rels.is_empty() || !fold_rels.is_empty() || !tag_rels.is_empty() {
        info!(
            "MySQL: 正在推送关联关系: 作者关系={}, 文件夹关系={}, 标签关系={}",
            auth_rels.len(),
            fold_rels.len(),
            tag_rels.len()
        );
        for r in auth_rels {
            debug!("MySQL: 推送作者关联: 文献ID={} <-> 作者ID={}", r.0, r.1);
            if let Err(e) = conn.exec_drop("INSERT INTO literature_authors (literature_id, author_id, sort_order, is_deleted, version, updated_at) VALUES (:lid, :aid, :so, :del, :v, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE sort_order=VALUES(sort_order), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()", params! { "lid" => &r.0, "aid" => &r.1, "so" => r.2, "del" => r.3, "v" => r.4 }).await {
                error!("MySQL: 推送作者关联失败 (文献: {}, 作者: {}): {}", r.0, r.1, e);
            } else if let Err(e) = db.mark_relation_synced("literature_authors", &r.0, &r.1) {
                error!("MySQL: 更新本地作者关联同步状态失败 (文献: {}, 作者: {}): {}", r.0, r.1, e);
            }
        }
        for r in fold_rels {
            debug!("MySQL: 推送文件夹关联: 文献ID={} <-> 文件夹ID={}", r.0, r.1);
            if let Err(e) = conn.exec_drop("INSERT INTO literature_folders (literature_id, folder_id, is_deleted, version, updated_at) VALUES (:lid, :fid, :del, :v, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()", params! { "lid" => &r.0, "fid" => &r.1, "del" => r.2, "v" => r.3 }).await {
                error!("MySQL: 推送文件夹关联失败 (文献: {}, 文件夹: {}): {}", r.0, r.1, e);
            } else if let Err(e) = db.mark_relation_synced("literature_folders", &r.0, &r.1) {
                error!("MySQL: 更新本地文件夹关联同步状态失败 (文献: {}, 文件夹: {}): {}", r.0, r.1, e);
            }
        }
        for r in tag_rels {
            debug!("MySQL: 推送标签关联: 文献ID={} <-> 标签ID={}", r.0, r.1);
            if let Err(e) = conn.exec_drop("INSERT INTO literature_tags (literature_id, tag_id, is_deleted, version, updated_at) VALUES (:lid, :tid, :del, :v, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()", params! { "lid" => &r.0, "tid" => &r.1, "del" => r.2, "v" => r.3 }).await {
                error!("MySQL: 推送标签关联失败 (文献: {}, 标签: {}): {}", r.0, r.1, e);
            } else if let Err(e) = db.mark_relation_synced("literature_tags", &r.0, &r.1) {
                error!("MySQL: 更新本地标签关联同步状态失败 (文献: {}, 标签: {}): {}", r.0, r.1, e);
            }
        }
    }

    let atts = db.get_dirty_attachments()?;
    if !atts.is_empty() {
        let total_dirty = atts.len();
        let filtered_atts: Vec<_> = if let Some(allowed_ids) = allowed_attachment_ids {
            atts.into_iter()
                .filter(|a| allowed_ids.contains(&a.id))
                .collect()
        } else {
            atts
        };

        if filtered_atts.len() < total_dirty {
            info!(
                "MySQL: 发现 {} 个脏附件记录，但只推送 {} 个成功上传到 WebDAV 的附件",
                total_dirty,
                filtered_atts.len()
            );
        } else {
            info!("MySQL: 正在推送 {} 个脏附件记录...", filtered_atts.len());
        }

        for a in filtered_atts {
            debug!("MySQL: 推送附件: {} (ID: {})", a.file_name, a.id);
            let abs_path = std::path::Path::new(&a.file_path);
            let rel_path_str = if let Ok(rel) = abs_path.strip_prefix(base_path) {
                rel.to_string_lossy().replace('\\', "/")
            } else {
                a.file_name.clone()
            };

            let q = "INSERT INTO attachments (id, literature_id, file_path, file_name, file_size, mime_type, etag, is_main, is_deleted, version, created_at, updated_at) VALUES (:id, :lit_id, :path, :name, :size, :mime, :etag, :is_main, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE file_path=VALUES(file_path), file_name=VALUES(file_name), file_size=VALUES(file_size), mime_type=VALUES(mime_type), etag=VALUES(etag), is_main=VALUES(is_main), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &a.id, "lit_id" => &a.literature_id, "path" => &rel_path_str, "name" => &a.file_name, "size" => a.file_size, "mime" => &a.mime_type, "etag" => &a.etag, "is_main" => a.is_main, "is_deleted" => a.is_deleted, "version" => a.version, "created_at" => &a.created_at }).await {
                error!("MySQL: 推送附件失败 '{}' (ID: {}): {}", a.file_name, a.id, e);
            } else if let Err(e) = db.mark_attachment_synced(&a.id) {
                error!("MySQL: 更新本地附件同步状态失败 (ID: {}): {}", a.id, e);
            }
        }
    }

    let feeds = db.get_dirty_feeds()?;
    if !feeds.is_empty() {
        info!("MySQL: 正在推送 {} 个脏订阅源记录...", feeds.len());
        for f in feeds {
            debug!("MySQL: 推送订阅源: {} (ID: {})", f.name, f.id);
            let normalized_last_up = f.last_updated_at.as_ref().map(|s| normalize_time_string(s));

            let q = "INSERT INTO feeds (id, name, feed_type, url, last_updated_at, update_interval, is_deleted, version, created_at, updated_at) VALUES (:id, :name, :type, :url, :last_up, :interval, :is_deleted, :version, :created_at, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE name=VALUES(name), feed_type=VALUES(feed_type), url=VALUES(url), last_updated_at=VALUES(last_updated_at), update_interval=VALUES(update_interval), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &f.id, "name" => &f.name, "type" => serde_json::to_string(&f.feed_type).unwrap_or_default().replace('"', ""), "url" => &f.url, "last_up" => &normalized_last_up, "interval" => f.update_interval, "is_deleted" => f.is_deleted, "version" => f.version, "created_at" => &f.created_at }).await {
                error!("MySQL: 推送订阅源失败 '{}' (ID: {}): {}", f.name, f.id, e);
            } else if let Err(e) = db.mark_feed_synced(&f.id) {
                error!("MySQL: 更新本地订阅源同步状态失败 (ID: {}): {}", f.id, e);
            }
        }
    }

    let items = db.get_dirty_feed_items()?;
    if !items.is_empty() {
        info!("MySQL: 正在推送 {} 个脏订阅条目记录...", items.len());
        for i in items {
            debug!("MySQL: 推送订阅条目: {} (ID: {})", i.title, i.id);
            let normalized_pub_at = i.published_at.as_ref().map(|s| normalize_time_string(s));

            let q = "INSERT INTO feed_items (id, title, feed_id, is_read, is_added_to_library, added_at, authors, year, type, journal, publisher, abstract_text, doi, url, volume, issue, pages, published_at, is_deleted, version, updated_at) VALUES (:id, :title, :fid, :read, :added, :added_at, :authors, :year, :type, :journal, :publisher, :abstract, :doi, :url, :vol, :issue, :pages, :pub_at, :is_deleted, :version, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE title=VALUES(title), is_read=VALUES(is_read), is_added_to_library=VALUES(is_added_to_library), authors=VALUES(authors), abstract_text=VALUES(abstract_text), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &i.id, "title" => &i.title, "fid" => &i.feed_id, "read" => i.is_read, "added" => i.is_added_to_library, "added_at" => &i.added_at, "authors" => serde_json::to_string(&i.authors).unwrap_or_default(), "year" => i.year, "type" => serde_json::to_string(&i.literature_type).unwrap_or_default().replace('"', ""), "journal" => &i.journal, "publisher" => &i.publisher, "abstract" => &i.abstract_text, "doi" => &i.doi, "url" => &i.url, "vol" => &i.volume, "issue" => &i.issue, "pages" => &i.pages, "pub_at" => &normalized_pub_at, "is_deleted" => i.is_deleted, "version" => i.version }).await {
                error!("MySQL: 推送订阅条目失败 '{}' (ID: {}): {}", i.title, i.id, e);
            } else if let Err(e) = db.mark_feed_item_synced(&i.id) {
                error!("MySQL: 更新本地订阅条目同步状态失败 (ID: {}): {}", i.id, e);
            }
        }
    }
    let citations = db.get_dirty_citations()?;
    if !citations.is_empty() {
        info!("MySQL: 正在推送 {} 个脏引用记录...", citations.len());
        for c in citations {
            debug!("MySQL: 推送引用: {} -> {}", c.source_id, c.target_id);
            let q = "INSERT INTO literature_citations (source_id, target_id, is_deleted, version, updated_at) VALUES (:sid, :tid, :is_deleted, :version, UNIX_TIMESTAMP()) ON DUPLICATE KEY UPDATE is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=UNIX_TIMESTAMP()";
            if let Err(e) = conn.exec_drop(q, params! { "sid" => &c.source_id, "tid" => &c.target_id, "is_deleted" => c.is_deleted, "version" => c.version }).await {
                error!("MySQL: 推送引用失败 '{} -> {}': {}", c.source_id, c.target_id, e);
            } else if let Err(e) = db.mark_citation_synced(&c.source_id, &c.target_id) {
                error!("MySQL: 更新本地引用同步状态失败 ({} -> {}): {}", c.source_id, c.target_id, e);
            }
        }
    }
    let annotations = db.get_dirty_annotations()?;
    if !annotations.is_empty() {
        info!("MySQL: 正在推送 {} 个脏注释记录...", annotations.len());
        for ann in annotations {
            debug!("MySQL: 推送注释: {}", ann.id);
            let kind_str = match ann.kind {
                models::AnnotationKind::Highlight => "Highlight",
                models::AnnotationKind::Underline => "Underline",
                models::AnnotationKind::Rectangle { .. } => "Rectangle",
            };
            let color_str = match ann.color {
                models::AnnotationColor::Yellow => "Yellow",
                models::AnnotationColor::Red => "Red",
                models::AnnotationColor::Green => "Green",
                models::AnnotationColor::Blue => "Blue",
                models::AnnotationColor::Purple => "Purple",
                models::AnnotationColor::Magenta => "Magenta",
                models::AnnotationColor::Orange => "Orange",
                models::AnnotationColor::Gray => "Gray",
            };
            let range_json = ann
                .range
                .as_ref()
                .and_then(|r| serde_json::to_string(r).ok());
            let (rx, ry, rw, rh) = match ann.kind {
                models::AnnotationKind::Rectangle { x, y, w, h } => {
                    (Some(x), Some(y), Some(w), Some(h))
                }
                _ => (None, None, None, None),
            };

            let q = "INSERT INTO annotations (id, document_id, page, kind, color, `range`, note, rect_x, rect_y, rect_w, rect_h, is_deleted, version, created_at, updated_at) VALUES (:id, :document_id, :page, :kind, :color, :range, :note, :rect_x, :rect_y, :rect_w, :rect_h, :is_deleted, :version, :created_at, :updated_at) ON DUPLICATE KEY UPDATE page=VALUES(page), kind=VALUES(kind), color=VALUES(color), `range`=VALUES(`range`), note=VALUES(note), rect_x=VALUES(rect_x), rect_y=VALUES(rect_y), rect_w=VALUES(rect_w), rect_h=VALUES(rect_h), is_deleted=VALUES(is_deleted), version=VALUES(version), updated_at=VALUES(updated_at)";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &ann.id, "document_id" => &ann.document_id, "page" => ann.page, "kind" => kind_str, "color" => color_str, "range" => range_json, "note" => &ann.note, "rect_x" => rx, "rect_y" => ry, "rect_w" => rw, "rect_h" => rh, "is_deleted" => ann.is_deleted, "version" => ann.version, "created_at" => ann.created_at, "updated_at" => ann.updated_at }).await {
                error!("MySQL: 推送注释失败 '{}': {}", ann.id, e);
            } else if let Err(e) = db.mark_annotation_synced(&ann.id) {
                error!("MySQL: 更新本地注释同步状态失败 ({}): {}", ann.id, e);
            }
        }
    }

    let notes = db.get_dirty_notes()?;
    if !notes.is_empty() {
        info!("MySQL: 正在推送 {} 个脏笔记记录...", notes.len());
        for note in notes {
            debug!("MySQL: 推送笔记: {} (ID: {})", note.title, note.id);
            let q = "INSERT INTO literature_notes (id, literature_id, title, content, sort_order, created_at, updated_at, is_deleted, version) VALUES (:id, :lit_id, :title, :content, :sort_order, :created_at, :updated_at, :is_deleted, :version) ON DUPLICATE KEY UPDATE literature_id=VALUES(literature_id), title=VALUES(title), content=VALUES(content), sort_order=VALUES(sort_order), updated_at=VALUES(updated_at), is_deleted=VALUES(is_deleted), version=VALUES(version)";
            if let Err(e) = conn.exec_drop(q, params! { "id" => &note.id, "lit_id" => &note.literature_id, "title" => &note.title, "content" => &note.content, "sort_order" => note.sort_order, "created_at" => note.created_at, "updated_at" => note.updated_at, "is_deleted" => note.is_deleted, "version" => note.version }).await {
                error!("MySQL: 推送笔记失败 '{}': {}", note.id, e);
            } else if let Err(e) = db.mark_note_synced(&note.id) {
                error!("MySQL: 更新本地笔记同步状态失败 ({}): {}", note.id, e);
            }
        }
    }

    Ok(())
}

async fn pull_remote_changes(
    conn: &mut mysql_async::Conn,
    db: Arc<Database>,
    base_path: &std::path::Path,
) -> Result<Vec<Literature>> {
    info!("MySQL: 正在增量拉取远程变更...");

    let mut conflicts = Vec::new();

    // ── authors ──
    let last_sync = db
        .get_last_sync_time("authors")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, first_name, last_name, middle_name, is_deleted, version, created_at, updated_at FROM authors WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程作者更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = AuthorRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_author(row.into_model())?;
        }
        db.set_last_sync_time("authors", &max_ua.to_string())?;
    }

    // ── folders ──
    let last_sync = db
        .get_last_sync_time("folders")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, name, folder_type, parent_id, is_deleted, version, created_at, updated_at FROM folders WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程文件夹更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = FolderRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_folder(row.into_model())?;
        }
        db.set_last_sync_time("folders", &max_ua.to_string())?;
    }

    // ── publications ──
    let last_sync = db
        .get_last_sync_time("publications")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, name, publication_type, abbreviation, publisher, ccf_rank, jcr_rank, cas_rank, is_deleted, version, created_at, updated_at FROM publications WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程出版源更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = PublicationRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_publication(row.into_model())?;
        }
        db.set_last_sync_time("publications", &max_ua.to_string())?;
    }

    // ── literatures ──
    let last_sync = db
        .get_last_sync_time("literatures")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT l.id, l.title, l.year, l.month, l.day, l.type, l.volume, l.issue, l.pages, l.abstract_text, l.doi, l.arxiv_id, l.url, l.rating, l.reading_status, l.is_deleted, l.version, l.created_at, l.updated_at, p.id, p.name, p.publication_type, p.abbreviation, p.publisher, p.ccf_rank, p.jcr_rank, p.cas_rank, p.is_deleted, p.version, p.created_at, p.updated_at FROM literatures l LEFT JOIN publications p ON l.publication_id = p.id WHERE l.updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程文献更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = LiteratureRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            let lit = row.into_literature();
            if let Some(conflict) = db.merge_remote_literature(lit)? {
                conflicts.push(conflict);
            }
        }
        db.set_last_sync_time("literatures", &max_ua.to_string())?;
    }

    // ── literature_authors ──
    let last_sync = db
        .get_last_sync_time("literature_authors")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT literature_id, author_id, sort_order, is_deleted, version, updated_at FROM literature_authors WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程作者关联更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let lid: Option<String> = r.get::<Option<String>, _>(0).flatten();
            let aid: Option<String> = r.get::<Option<String>, _>(1).flatten();
            let sort_order: Option<i32> = r.get::<Option<i32>, _>(2).flatten();
            let is_deleted: Option<bool> = r.get::<Option<bool>, _>(3).flatten();
            let version: Option<i32> = r.get::<Option<i32>, _>(4).flatten();
            let ua: Option<i64> = r.get::<Option<i64>, _>(5).flatten();
            if let Some(ua) = ua
                && ua > max_ua
            {
                max_ua = ua;
            }
            if let (Some(lid), Some(aid)) = (lid, aid) {
                db.merge_remote_relation(
                    "literature_authors",
                    lid,
                    aid,
                    sort_order,
                    is_deleted.unwrap_or(false),
                    version.unwrap_or(1),
                )?;
            }
        }
        db.set_last_sync_time("literature_authors", &max_ua.to_string())?;
    }

    // ── literature_folders ──
    let last_sync = db
        .get_last_sync_time("literature_folders")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT literature_id, folder_id, is_deleted, version, updated_at FROM literature_folders WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程文件夹关联更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let lid: Option<String> = r.get::<Option<String>, _>(0).flatten();
            let fid: Option<String> = r.get::<Option<String>, _>(1).flatten();
            let is_deleted: Option<bool> = r.get::<Option<bool>, _>(2).flatten();
            let version: Option<i32> = r.get::<Option<i32>, _>(3).flatten();
            let ua: Option<i64> = r.get::<Option<i64>, _>(4).flatten();
            if let Some(ua) = ua
                && ua > max_ua
            {
                max_ua = ua;
            }
            if let (Some(lid), Some(fid)) = (lid, fid) {
                db.merge_remote_relation(
                    "literature_folders",
                    lid,
                    fid,
                    None,
                    is_deleted.unwrap_or(false),
                    version.unwrap_or(1),
                )?;
            }
        }
        db.set_last_sync_time("literature_folders", &max_ua.to_string())?;
    }

    // ── literature_tags ──
    let last_sync = db
        .get_last_sync_time("literature_tags")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT literature_id, tag_id, is_deleted, version, updated_at FROM literature_tags WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程标签关联更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let lid: Option<String> = r.get::<Option<String>, _>(0).flatten();
            let tid: Option<String> = r.get::<Option<String>, _>(1).flatten();
            let is_deleted: Option<bool> = r.get::<Option<bool>, _>(2).flatten();
            let version: Option<i32> = r.get::<Option<i32>, _>(3).flatten();
            let ua: Option<i64> = r.get::<Option<i64>, _>(4).flatten();
            if let Some(ua) = ua
                && ua > max_ua
            {
                max_ua = ua;
            }
            if let (Some(lid), Some(tid)) = (lid, tid) {
                db.merge_remote_relation(
                    "literature_tags",
                    lid,
                    tid,
                    None,
                    is_deleted.unwrap_or(false),
                    version.unwrap_or(1),
                )?;
            }
        }
        db.set_last_sync_time("literature_tags", &max_ua.to_string())?;
    }

    // ── attachments ──
    let last_sync = db
        .get_last_sync_time("attachments")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, literature_id, file_path, file_name, file_size, mime_type, etag, is_main, is_deleted, version, created_at, updated_at FROM attachments WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程附件更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = AttachmentRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_attachment(row.into_model(base_path))?;
        }
        db.set_last_sync_time("attachments", &max_ua.to_string())?;
    }

    // ── feeds ──
    let last_sync = db
        .get_last_sync_time("feeds")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, name, feed_type, url, last_updated_at, update_interval, is_deleted, version, created_at, updated_at FROM feeds WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程订阅源更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = FeedRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_feed(row.into_model())?;
        }
        db.set_last_sync_time("feeds", &max_ua.to_string())?;
    }

    // ── feed_items ──
    let last_sync = db
        .get_last_sync_time("feed_items")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, title, feed_id, is_read, is_added_to_library, added_at, authors, year, type, journal, publisher, abstract_text, doi, url, volume, issue, pages, published_at, is_deleted, version, updated_at FROM feed_items WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程订阅条目更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let row = FeedItemRow::from_mysql_row(r)?;
            if let Some(ua) = row.updated_at
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_feed_item(row.into_model())?;
        }
        db.set_last_sync_time("feed_items", &max_ua.to_string())?;
    }

    // ── literature_citations ──
    let last_sync = db
        .get_last_sync_time("literature_citations")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT source_id, target_id, is_deleted, version, updated_at FROM literature_citations WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程引用更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for r in rows {
            let ua: Option<i64> = r.get::<Option<i64>, _>(4).flatten();
            if let Some(ua) = ua
                && ua > max_ua
            {
                max_ua = ua;
            }
            db.merge_remote_citation(CitationRow::from_mysql_row(r)?.into_model())?;
        }
        db.set_last_sync_time("literature_citations", &max_ua.to_string())?;
    }

    // ── annotations (BIGINT updated_at) ──
    let last_sync = db
        .get_last_sync_time("annotations")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, document_id, page, kind, color, `range`, note, rect_x, rect_y, rect_w, rect_h, created_at, updated_at, version, is_deleted FROM annotations WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程注释更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for mut r in rows {
            let kind_str: String = r.take("kind").unwrap_or_default();
            let color_str: String = r.take("color").unwrap_or_default();
            let range_str: Option<String> = r.take("range");
            let rect_x: Option<f32> = r.take("rect_x");
            let rect_y: Option<f32> = r.take("rect_y");
            let rect_w: Option<f32> = r.take("rect_w");
            let rect_h: Option<f32> = r.take("rect_h");

            let kind = match kind_str.as_str() {
                "Highlight" => models::AnnotationKind::Highlight,
                "Underline" => models::AnnotationKind::Underline,
                "Rectangle" => models::AnnotationKind::Rectangle {
                    x: rect_x.unwrap_or(0.0),
                    y: rect_y.unwrap_or(0.0),
                    w: rect_w.unwrap_or(0.0),
                    h: rect_h.unwrap_or(0.0),
                },
                _ => models::AnnotationKind::Highlight,
            };

            let color = match color_str.as_str() {
                "Yellow" => models::AnnotationColor::Yellow,
                "Red" => models::AnnotationColor::Red,
                "Green" => models::AnnotationColor::Green,
                "Blue" => models::AnnotationColor::Blue,
                "Purple" => models::AnnotationColor::Purple,
                "Magenta" => models::AnnotationColor::Magenta,
                "Orange" => models::AnnotationColor::Orange,
                "Gray" => models::AnnotationColor::Gray,
                _ => models::AnnotationColor::Yellow,
            };

            let created_at: i64 = r.take("created_at").unwrap_or(0);
            let updated_at: i64 = r.take("updated_at").unwrap_or(0);
            if updated_at > max_ua {
                max_ua = updated_at;
            }

            let ann = models::Annotation {
                id: r.take("id").unwrap_or_default(),
                document_id: r.take("document_id").unwrap_or_default(),
                page: r.take("page").unwrap_or(0),
                kind,
                color,
                range: range_str.and_then(|s| serde_json::from_str(&s).ok()),
                note: r.take("note"),
                created_at,
                updated_at,
                version: r.take::<Option<i32>, _>("version").flatten().unwrap_or(1),
                is_deleted: r
                    .take::<Option<bool>, _>("is_deleted")
                    .flatten()
                    .unwrap_or(false),
                is_dirty: false,
            };
            db.merge_remote_annotation(ann)?;
        }
        db.set_last_sync_time("annotations", &max_ua.to_string())?;
    }

    // ── literature_notes (BIGINT updated_at) ──
    let last_sync = db
        .get_last_sync_time("literature_notes")?
        .unwrap_or_else(|| "0".to_string());
    let rows: Vec<mysql_async::Row> = conn.exec("SELECT id, literature_id, title, content, sort_order, created_at, updated_at, is_deleted, version FROM literature_notes WHERE updated_at > :t", params! { "t" => &last_sync }).await?;
    if !rows.is_empty() {
        info!("MySQL: 发现 {} 条远程笔记更新", rows.len());
        let mut max_ua: i64 = last_sync.parse().unwrap_or(0);
        for mut r in rows {
            let updated_at: i64 = r.take("updated_at").unwrap_or(0);
            if updated_at > max_ua {
                max_ua = updated_at;
            }
            let note = models::LiteratureNote {
                id: r.take("id").unwrap_or_default(),
                literature_id: r.take("literature_id").unwrap_or_default(),
                title: r.take("title").unwrap_or_default(),
                content: r.take("content").unwrap_or_default(),
                sort_order: r.take("sort_order").unwrap_or(0),
                created_at: r.take("created_at").unwrap_or(0),
                updated_at,
                is_deleted: r
                    .take::<Option<bool>, _>("is_deleted")
                    .flatten()
                    .unwrap_or(false),
                is_dirty: false,
                version: r.take::<Option<i32>, _>("version").flatten().unwrap_or(1),
            };
            db.merge_remote_note(note)?;
        }
        db.set_last_sync_time("literature_notes", &max_ua.to_string())?;
    }

    Ok(conflicts)
}

fn parse_time_string(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Some(dt.naive_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt);
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%d %b %Y %H:%M:%S") {
        return Some(dt);
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%d %b %Y") {
        return d.and_hms_opt(0, 0, 0);
    }

    None
}

fn normalize_time_string(s: &str) -> String {
    if let Some(dt) = parse_time_string(s) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        s.to_string()
    }
}
