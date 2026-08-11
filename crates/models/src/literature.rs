use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum ReadingStatus {
    #[default]
    Unread,
    ToRead,
    Reading,
    Read,
}

impl Display for ReadingStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            ReadingStatus::Unread => write!(f, "Unread"),
            ReadingStatus::ToRead => write!(f, "ToRead"),
            ReadingStatus::Reading => write!(f, "Reading"),
            ReadingStatus::Read => write!(f, "Read"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
pub enum LiteratureType {
    #[default]
    #[serde(rename = "article")]
    Article,
    #[serde(rename = "book")]
    Book,
    #[serde(rename = "conference")]
    Conference,
    #[serde(rename = "thesis")]
    Thesis,
    #[serde(rename = "preprint")]
    Preprint,
    #[serde(rename = "technical_report")]
    TechnicalReport,
    #[serde(rename = "webpage")]
    Webpage,
    #[serde(rename = "other")]
    Other,
}

impl Display for LiteratureType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            LiteratureType::Article => write!(f, "Article"),
            LiteratureType::Book => write!(f, "Book"),
            LiteratureType::Conference => write!(f, "Conference"),
            LiteratureType::Thesis => write!(f, "Thesis"),
            LiteratureType::Preprint => write!(f, "Preprint"),
            LiteratureType::TechnicalReport => write!(f, "Technical Report"),
            LiteratureType::Webpage => write!(f, "Webpage"),
            LiteratureType::Other => write!(f, "Other"),
        }
    }
}

impl LiteratureType {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "article" => Some(Self::Article),
            "book" => Some(Self::Book),
            "conference" => Some(Self::Conference),
            "thesis" => Some(Self::Thesis),
            "preprint" => Some(Self::Preprint),
            "technical_report" => Some(Self::TechnicalReport),
            "webpage" => Some(Self::Webpage),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Book => "book",
            Self::Conference => "conference",
            Self::Thesis => "thesis",
            Self::Preprint => "preprint",
            Self::TechnicalReport => "technical_report",
            Self::Webpage => "webpage",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Literature {
    pub id: String,
    pub title: String,
    pub authors: Vec<crate::Author>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub literature_type: LiteratureType,
    pub publication: Option<crate::Publication>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub abstract_text: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub url: Option<String>,
    pub tags: Vec<String>,
    pub rating: i32,
    pub folder_ids: Vec<String>,
    pub attachments: Vec<crate::Attachment>,
    pub reading_status: ReadingStatus,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: i64,
    pub updated_at: i64,
}
