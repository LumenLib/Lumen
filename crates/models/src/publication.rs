use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PublicationType {
    #[serde(rename = "journal")]
    Journal,
    #[serde(rename = "conference")]
    Conference,
    #[serde(rename = "book")]
    Book,
}

impl Display for PublicationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            PublicationType::Journal => write!(f, "journal"),
            PublicationType::Conference => write!(f, "conference"),
            PublicationType::Book => write!(f, "book"),
        }
    }
}

impl FromStr for PublicationType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "journal" => Ok(PublicationType::Journal),
            "conference" => Ok(PublicationType::Conference),
            "book" => Ok(PublicationType::Book),
            _ => Ok(PublicationType::Journal),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Publication {
    pub id: String,
    pub name: String,
    pub publication_type: PublicationType,
    pub abbreviation: Option<String>,
    pub publisher: Option<String>,
    pub ccf_rank: Option<String>,
    pub jcr_rank: Option<String>,
    pub cas_rank: Option<String>,
    pub is_dirty: bool,
    pub is_deleted: bool,
    pub version: i32,
    pub created_at: i64,
    pub updated_at: i64,
}
