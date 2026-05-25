use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Citation {
    pub source_id: String,
    pub target_id: String,
    pub is_deleted: bool,
    pub version: i64,
    pub updated_at: String,
}
