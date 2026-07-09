pub mod claude;
pub mod compression;
pub mod ollama;
pub mod openai;
pub mod siliconflow;
pub mod types;

use anyhow::Result;
use log::{debug, error};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub use types::*;

#[derive(Clone)]
pub struct AiService {
    backend: Arc<dyn AiBackend>,
    backend_kind: BackendKind,
    model: String,
}

impl AiService {
    pub fn new(kind: BackendKind, config: &AiConfig) -> Self {
        let backend_name = match kind {
            BackendKind::OpenAI => "OpenAI",
            BackendKind::Ollama => "Ollama",
            BackendKind::Claude => "Claude",
            BackendKind::SiliconFlow => "SiliconFlow",
        };
        debug!(
            "AiService::new: backend={}, model={}, api_base={}",
            backend_name, config.model, config.api_base,
        );
        let backend: Arc<dyn AiBackend> = match kind {
            BackendKind::OpenAI => Arc::new(openai::OpenAiBackend::new(config)),
            BackendKind::Ollama => Arc::new(ollama::OllamaBackend::new(config)),
            BackendKind::Claude => Arc::new(claude::ClaudeBackend::new(config)),
            BackendKind::SiliconFlow => Arc::new(siliconflow::SiliconFlowBackend::new(config)),
        };
        Self {
            backend,
            backend_kind: kind,
            model: config.model.clone(),
        }
    }

    pub fn name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn backend_kind(&self) -> BackendKind {
        self.backend_kind
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub async fn chat(&self, messages: &[ChatMessage], system: Option<&str>) -> Result<String> {
        let msg_count = messages.len();
        let text_len: usize = messages.iter().map(|m| m.content.len()).sum();
        debug!(
            "AiService::chat: backend={}, model={}, messages={}, chars={}, system={}",
            self.backend.name(),
            self.model,
            msg_count,
            text_len,
            system.is_some(),
        );
        let result = self.backend.chat(messages, system).await;
        match &result {
            Ok(s) => debug!("AiService::chat: 成功, result_len={}", s.len()),
            Err(e) => error!("AiService::chat: 失败: {e}"),
        }
        result
    }

    pub fn chat_stream(
        &self,
        messages: &[ChatMessage],
        system: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<ChatResponseStream>> + Send>> {
        self.backend.chat_stream(messages, system)
    }
}
