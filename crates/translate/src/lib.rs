use anyhow::Result;
use log::{debug, info};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub mod bing_free;
pub mod google;
pub mod google_free;
pub mod niutrans;

pub trait TranslationBackend: Send + Sync {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;
    fn name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct TranslationService {
    backend: Arc<dyn TranslationBackend>,
}

impl TranslationService {
    pub fn new(engine: &str, keys: &HashMap<String, String>) -> Self {
        info!("TranslationService: 创建翻译服务, 引擎={}", engine);
        let backend: Arc<dyn TranslationBackend> = match engine {
            "google_free" => Arc::new(google_free::GoogleFreeBackend::new()),
            "bing_free" => Arc::new(bing_free::BingFreeBackend::new()),
            "google" => Arc::new(google::GoogleBackend::new(
                keys.get("google").cloned().unwrap_or_default(),
            )),
            "niutrans" => Arc::new(niutrans::NiuTransBackend::new(
                keys.get("niutrans").cloned().unwrap_or_default(),
            )),
            _ => Arc::new(google_free::GoogleFreeBackend::new()),
        };
        Self { backend }
    }

    pub async fn translate(&self, text: &str, target_lang: &str) -> Result<String> {
        debug!(
            "TranslationService: 翻译, 引擎={}, 目标语言={}, 文本长度={}",
            self.backend.name(),
            target_lang,
            text.len()
        );
        let result = self.backend.translate(text, target_lang).await;
        match &result {
            Ok(t) => debug!("TranslationService: 翻译完成, 结果长度={}", t.len()),
            Err(e) => log::error!("TranslationService: 翻译失败: {}", e),
        }
        result
    }

    pub fn engine_name(&self) -> &'static str {
        self.backend.name()
    }
}
