use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::{Client, Response};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct GoogleFreeBackend {
    client: Client,
}

impl GoogleFreeBackend {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }
}

impl crate::TranslationBackend for GoogleFreeBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let client = self.client.clone();
        let text = text.to_string();
        let target_lang = target_lang.to_string();

        Box::pin(async move {
            debug!(
                "GoogleFreeBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );
            let url = format!(
                "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&ie=UTF-8&oe=UTF-8&q={}",
                target_lang,
                urlencoding::encode(&text)
            );
            let resp: Response = client.get(url).send().await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                error!("GoogleFreeBackend: 请求失败: {} - {}", status, body);
                return Err(anyhow!(
                    "Google Translate request failed: {} - {}",
                    status,
                    body
                ));
            }

            let json: Value = resp.json().await?;

            let mut result = String::new();
            if let Some(sentences) = json.get(0).and_then(|v| v.as_array()) {
                for sentence_info in sentences {
                    if let Some(translated) = sentence_info.get(0).and_then(|v| v.as_str()) {
                        result.push_str(translated);
                    }
                }
            }

            if result.is_empty() {
                error!("GoogleFreeBackend: 响应解析失败: {:?}", json);
                Err(anyhow!(
                    "Failed to extract translation from Google response: {:?}",
                    json
                ))
            } else {
                debug!("GoogleFreeBackend: 翻译成功");
                Ok(result)
            }
        })
    }
}
