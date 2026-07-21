use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Author {
    pub id: String,
    pub last_name: String,
    pub first_name: String,
    pub middle_name: Option<String>,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: i64,
    pub updated_at: i64,
}
