use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnnotationColor {
    Yellow,
    Red,
    Green,
    Blue,
    Purple,
    Magenta,
    Orange,
    Gray,
}

impl AnnotationColor {
    pub fn to_hex(&self) -> &'static str {
        match self {
            Self::Yellow => "#ffc90e",
            Self::Red => "#fa5a5a",
            Self::Green => "#4bb23a",
            Self::Blue => "#2aa6df",
            Self::Purple => "#9b88e5",
            Self::Magenta => "#e55ce6",
            Self::Orange => "#f08c28",
            Self::Gray => "#a6a6a6",
        }
    }

    pub fn to_hsla(&self) -> gpui::Hsla {
        gpui::rgb(u32::from_str_radix(&self.to_hex()[1..], 16).unwrap_or(0)).into()
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
