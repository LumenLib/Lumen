use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteratureNote {
    pub id: String,
    pub literature_id: String,
    pub title: String,
    pub content: String,
    pub sort_order: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub is_deleted: bool,
    pub is_dirty: bool,
    pub version: i32,
}
