use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnnotationColor {
    Yellow,
    Green,
    Blue,
    Pink,
    Orange,
    Purple,
    Red,
    Cyan,
}

impl AnnotationColor {
    pub fn to_hex(&self) -> &'static str {
        match self {
            Self::Yellow => "#FFD700",
            Self::Green => "#90EE90",
            Self::Blue => "#87CEEB",
            Self::Pink => "#FFB6C1",
            Self::Orange => "#FFA500",
            Self::Purple => "#DDA0DD",
            Self::Red => "#FF6B6B",
            Self::Cyan => "#7FFFD4",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnnotationKind {
    Highlight,
    Underline,
    Rectangle { x: f32, y: f32, w: f32, h: f32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRange {
    #[serde(alias = "page")]
    pub start_page: u16,
    pub start_char: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_page: Option<u16>,
    pub end_char: usize,
}

impl TextRange {
    pub fn end_page_or(&self) -> u16 {
        self.end_page.unwrap_or(self.start_page)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub document_id: String,
    pub page: u16,
    pub kind: AnnotationKind,
    pub color: AnnotationColor,
    pub range: Option<TextRange>,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub version: i32,
    pub is_deleted: bool,
    pub is_dirty: bool,
}
