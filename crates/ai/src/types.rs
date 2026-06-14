use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    pub file_path: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub extracted_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    pub attachments: Vec<AttachmentInfo>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            attachments: Vec::new(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::Assistant,
            content: content.into(),
            attachments: Vec::new(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: ChatRole::System,
            content: content.into(),
            attachments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    OpenAI,
    Ollama,
    Claude,
}

impl BackendKind {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => Self::OpenAI,
            "ollama" => Self::Ollama,
            "claude" => Self::Claude,
            _ => Self::OpenAI,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiConfig {
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub context_window: u32,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_base: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.3,
            max_tokens: 4096,
            context_window: 128000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiBackendEntry {
    pub name: String,
    pub kind: String,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    #[serde(default = "default_context_window")]
    pub context_window: u32,
    #[serde(default = "default_compression_strategy")]
    pub compression_strategy: String,
}

fn default_context_window() -> u32 {
    128000
}

fn default_compression_strategy() -> String {
    "sliding_window".into()
}

impl AiBackendEntry {
    pub fn to_config(&self) -> AiConfig {
        AiConfig {
            api_key: self.api_key.clone(),
            api_base: self.api_base.clone(),
            model: self.model.clone(),
            temperature: self.temperature,
            max_tokens: self.max_tokens,
            context_window: self.context_window,
        }
    }
}

pub struct ChatResponseStream {
    rx: mpsc::UnboundedReceiver<Result<String>>,
}

impl ChatResponseStream {
    pub fn new(rx: mpsc::UnboundedReceiver<Result<String>>) -> Self {
        Self { rx }
    }
}

impl futures_core::Stream for ChatResponseStream {
    type Item = Result<String>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

pub trait AiBackend: Send + Sync {
    fn chat(
        &self,
        messages: &[ChatMessage],
        system: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;

    fn chat_stream(
        &self,
        messages: &[ChatMessage],
        system: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatResponseStream>> + Send>>;

    fn name(&self) -> &'static str;
}
