use serde::{Deserialize, Serialize};

pub const DEFAULT_TAG_COLOR: &str = "#3b82f6";

pub const TAG_COLORS: &[(&str, &str)] = &[
    ("Red", "#ef4444"),
    ("Orange", "#f97316"),
    ("Yellow", "#eab308"),
    ("Green", "#22c55e"),
    ("Cyan", "#06b6d4"),
    ("Blue", "#3b82f6"),
    ("Indigo", "#6366f1"),
    ("Purple", "#a855f7"),
    ("Pink", "#ec4899"),
    ("Slate", "#64748b"),
];

pub const DEFAULT_VERSION: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i32,
    pub is_deleted: bool,
    #[serde(skip)]
    pub is_dirty: bool,
}
