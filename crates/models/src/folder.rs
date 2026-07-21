use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FolderType {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "custom")]
    Custom,
    #[serde(rename = "uncategorized")]
    Uncategorized,
    #[serde(rename = "trash")]
    Trash,
}

impl Display for FolderType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            FolderType::All => write!(f, "All"),
            FolderType::Custom => write!(f, "Custom"),
            FolderType::Uncategorized => write!(f, "Uncategorized"),
            FolderType::Trash => write!(f, "Trash"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub folder_type: FolderType,
    pub parent_id: Option<String>,
    pub literature_count: usize,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: i64,
    pub updated_at: i64,
}
