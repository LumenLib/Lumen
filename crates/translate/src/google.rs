use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::Client;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct GoogleBackend {
    client: Client,
    api_key: String,
}

impl GoogleBackend {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent(crate::CHROME_UA)
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_key,
        }
    }
}

impl crate::TranslationBackend for GoogleBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let text = text.to_string();
        let target_lang = target_lang.to_string();

        Box::pin(async move {
            if api_key.is_empty() {
                error!("GoogleBackend: API Key 未配置");
                return Err(anyhow!("Google API Key is not configured"));
            }

            debug!(
                "GoogleBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );
            let url = format!(
                "https://translation.googleapis.com/language/translate/v2?key={}",
                api_key
            );
            let resp = client
                .post(url)
                .json(&serde_json::json!({
                    "q": [text],
                    "target": target_lang
                }))
                .send()
                .await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                error!("GoogleBackend: 请求失败: {} - {}", status, body);
                return Err(anyhow!(
                    "Google Cloud Translation failed: {} - {}",
                    status,
                    body
                ));
            }

            let json: Value = resp.json().await?;
            let translated = json["data"]["translations"][0]["translatedText"]
                .as_str()
                .ok_or_else(|| {
                    error!("GoogleBackend: 响应解析失败");
                    anyhow!("Failed to parse Google Cloud response")
                })?;

            debug!("GoogleBackend: 翻译成功");
            Ok(translated.to_string())
        })
    }
}
