use anyhow::{Result, anyhow};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub trait AiBackend: Send + Sync {
    fn chat(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;
    fn name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct AiService {
    backend: Arc<dyn AiBackend>,
}

struct NoneBackend;

impl AiBackend for NoneBackend {
    fn chat(
        &self,
        _prompt: &str,
        _system: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        Box::pin(async move { Err(anyhow!("AI engine not configured")) })
    }

    fn name(&self) -> &'static str {
        "none"
    }
}

impl AiService {
    pub fn new(engine: &str) -> Self {
        let backend: Arc<dyn AiBackend> = match engine {
            _ => Arc::new(NoneBackend),
        };
        Self { backend }
    }

    pub async fn chat(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        self.backend.chat(prompt, system).await
    }

    pub fn engine_name(&self) -> &'static str {
        self.backend.name()
    }
}
