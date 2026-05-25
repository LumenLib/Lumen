use serde::{Deserialize, Serialize};

use crate::{Author, LiteratureType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub id: String,
    pub title: String,
    pub authors: Vec<Author>,
    pub year: Option<i32>,
    pub literature_type: LiteratureType,
    pub journal: Option<String>,
    pub publisher: Option<String>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub feed_id: String,
    pub added_at: String,
    pub published_at: Option<String>,
    pub is_read: bool,
    pub is_added_to_library: bool,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeedType {
    Journal,
    Conference,
    Rss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub id: String,
    pub name: String,
    pub feed_type: FeedType,
    pub url: Option<String>,
    pub unread_count: usize,
    pub total_count: usize,
    pub update_interval: u32,
    pub last_updated_at: Option<String>,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}
