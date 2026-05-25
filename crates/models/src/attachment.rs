use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    pub id: String,
    pub literature_id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub mime_type: Option<String>,
    pub etag: Option<String>,
    pub is_main: bool,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: String,
    pub updated_at: String,
}
