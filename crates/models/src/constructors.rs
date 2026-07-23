//! 构造器函数
//!
//! 原 `impl Struct { pub fn new(...) }` 改为自由函数，与类型同处 models 层。
//! 由 database/src/constructors.rs 迁回此处（A0 重构）。

use crate::{
    Attachment, Author, DEFAULT_TAG_COLOR, DEFAULT_VERSION, Feed, FeedItem, FeedType, Folder,
    FolderType, Literature, LiteratureType, Publication, PublicationType, ReadingStatus, Tag,
};
use chrono::Local;
use uuid::Uuid;

// ── Literature ──

pub fn create_literature(
    id: impl Into<String>,
    title: impl Into<String>,
    lit_type: LiteratureType,
) -> Literature {
    let now = Local::now().timestamp();
    Literature {
        id: id.into(),
        title: title.into(),
        authors: Vec::new(),
        year: None,
        month: None,
        day: None,
        literature_type: lit_type,
        publication: None,
        volume: None,
        issue: None,
        pages: None,
        abstract_text: None,
        doi: None,
        arxiv_id: None,
        url: None,
        tags: Vec::new(),
        rating: 0,
        folder_ids: Vec::new(),
        attachments: Vec::new(),
        reading_status: ReadingStatus::Unread,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

// ── Folder ──

pub fn create_folder(
    id: impl Into<String>,
    name: impl Into<String>,
    folder_type: FolderType,
) -> Folder {
    let now = Local::now().timestamp();
    Folder {
        id: id.into(),
        name: name.into(),
        folder_type,
        parent_id: None,
        literature_count: 0,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

// ── Feed ──

pub fn create_feed(id: impl Into<String>, name: impl Into<String>, feed_type: FeedType) -> Feed {
    let now = Local::now().timestamp();
    Feed {
        id: id.into(),
        name: name.into(),
        title: None,
        feed_type,
        url: None,
        unread_count: 0,
        total_count: 0,
        update_interval: 24,
        last_updated_at: None,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

pub fn create_feed_item(
    id: impl Into<String>,
    title: impl Into<String>,
    feed_id: impl Into<String>,
) -> FeedItem {
    let now = Local::now().timestamp();
    FeedItem {
        id: id.into(),
        title: title.into(),
        authors: Vec::new(),
        year: None,
        literature_type: LiteratureType::Article,
        journal: None,
        publisher: None,
        abstract_text: None,
        doi: None,
        url: None,
        volume: None,
        issue: None,
        pages: None,
        feed_id: feed_id.into(),
        added_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        published_at: None,
        is_read: false,
        is_added_to_library: false,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        updated_at: now,
    }
}

// ── Publication ──

pub fn create_publication(name: impl Into<String>, pub_type: PublicationType) -> Publication {
    let now = Local::now().timestamp();
    Publication {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        publication_type: pub_type,
        abbreviation: None,
        publisher: None,
        ccf_rank: None,
        jcr_rank: None,
        cas_rank: None,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

// ── Tag ──

pub fn create_tag(name: impl Into<String>) -> Tag {
    let now = Local::now().timestamp();
    Tag {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        color: DEFAULT_TAG_COLOR.to_string(),
        created_at: now,
        updated_at: now,
        version: DEFAULT_VERSION,
        is_deleted: false,
        is_dirty: true,
    }
}

pub fn create_tag_with_color(name: impl Into<String>, color: impl Into<String>) -> Tag {
    let mut tag = create_tag(name);
    tag.color = color.into();
    tag
}

// ── Author ──

pub fn create_author(last_name: impl Into<String>, first_name: impl Into<String>) -> Author {
    let now = Local::now().timestamp();
    Author {
        id: Uuid::new_v4().to_string(),
        last_name: last_name.into(),
        first_name: first_name.into(),
        middle_name: None,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}

// ── Attachment ──

pub fn create_attachment(
    id: String,
    literature_id: String,
    file_path: String,
    file_name: String,
    file_size: u64,
) -> Attachment {
    let now = Local::now().timestamp();
    Attachment {
        id,
        literature_id,
        file_path,
        file_name,
        file_size,
        mime_type: None,
        etag: None,
        hash: None,
        is_main: false,
        is_dirty: true,
        is_deleted: false,
        version: 1,
        created_at: now,
        updated_at: now,
    }
}
