use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub literature_id: String,
    pub title: String,
    pub system_prompt: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub compressed_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    pub attachments: Vec<String>,
    pub created_at: i64,
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChatResponseChunk {
    Content(String),
    Reasoning(String),
}
