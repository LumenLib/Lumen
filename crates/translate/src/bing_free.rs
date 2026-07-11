use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct BingFreeBackend {
    client: Client,
    token_cache: Arc<RwLock<Option<(String, Instant)>>>,
}

#[derive(Serialize)]
struct BingTranslateRequest<'a> {
    #[serde(rename = "Text")]
    text: &'a str,
}

#[derive(Deserialize)]
struct BingTranslateResponse {
    translations: Vec<BingTranslation>,
}

#[derive(Deserialize)]
struct BingTranslation {
    text: String,
}

impl Default for BingFreeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl BingFreeBackend {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0")
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            token_cache: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_token(
        client: &Client,
        token_cache: &Arc<RwLock<Option<(String, Instant)>>>,
    ) -> Result<String> {
        {
            let cache = token_cache.read().await;
            if let Some((token, expiry)) = &*cache
                && Instant::now() < *expiry
            {
                return Ok(token.clone());
            }
        }

        debug!("BingFreeBackend: 获取 token");
        let url = "https://edge.microsoft.com/translate/auth";
        let resp = client.get(url).send().await?;
        if !resp.status().is_success() {
            error!("BingFreeBackend: 获取 token 失败: {}", resp.status());
            return Err(anyhow!("Failed to get Bing auth token: {}", resp.status()));
        }
        let token = resp.text().await?;

        {
            let mut cache = token_cache.write().await;
            *cache = Some((token.clone(), Instant::now() + Duration::from_secs(5 * 60)));
        }

        Ok(token)
    }
}

impl crate::TranslationBackend for BingFreeBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let client = self.client.clone();
        let token_cache = self.token_cache.clone();
        let text = text.to_string();
        let target_lang = target_lang.to_string();

        Box::pin(async move {
            debug!(
                "BingFreeBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );
            let token = Self::get_token(&client, &token_cache).await?;

            let url = format!(
                "https://api-edge.cognitive.microsofttranslator.com/translate?to={}&api-version=3.0&includeSentenceLength=true",
                target_lang
            );

            let body = vec![BingTranslateRequest { text: &text }];

            let resp = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Referer", "https://appsumo.com/")
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                error!("BingFreeBackend: 请求失败: {} - {}", status, body_text);
                return Err(anyhow!("Bing Translate failed: {} - {}", status, body_text));
            }

            let json: Vec<BingTranslateResponse> = resp.json().await?;
            let translated = json
                .first()
                .and_then(|r| r.translations.first())
                .map(|t| t.text.clone())
                .ok_or_else(|| {
                    error!("BingFreeBackend: 响应解析失败");
                    anyhow!("Failed to parse Bing response")
                })?;

            debug!("BingFreeBackend: 翻译成功");
            Ok(translated)
        })
    }
}
