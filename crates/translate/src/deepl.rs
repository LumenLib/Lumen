use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct DeeplBackend {
    client: Client,
    api_key: String,
    is_pro: bool,
}

#[derive(Serialize)]
struct DeeplRequest {
    text: Vec<String>,
    target_lang: String,
}

#[derive(Deserialize)]
struct DeeplTranslation {
    text: String,
}

#[derive(Deserialize)]
struct DeeplResponse {
    translations: Vec<DeeplTranslation>,
}

impl DeeplBackend {
    pub fn new(api_key: String, is_pro: bool) -> Self {
        Self {
            client: Client::builder()
                .user_agent(crate::CHROME_UA)
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_key,
            is_pro,
        }
    }
}

fn map_lang(lang: &str) -> String {
    match lang {
        "zh-CN" | "zh-SG" => "ZH-HANS".into(),
        "zh-TW" | "zh-HK" | "zh-MO" => "ZH-HANT".into(),
        "en-GB" => "EN-GB".into(),
        "en-US" => "EN-US".into(),
        "pt-BR" => "PT-BR".into(),
        "pt-PT" => "PT-PT".into(),
        _ => lang.split('-').next().unwrap_or(lang).to_uppercase(),
    }
}

impl crate::TranslationBackend for DeeplBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let is_pro = self.is_pro;
        let text = text.to_string();
        let target_lang = target_lang.to_string();

        Box::pin(async move {
            if api_key.is_empty() {
                error!("DeeplBackend: API Key 未配置");
                return Err(anyhow!("DeepL API Key is not configured"));
            }

            debug!(
                "DeeplBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );

            let url = if is_pro {
                "https://api.deepl.com/v2/translate"
            } else {
                "https://api-free.deepl.com/v2/translate"
            };

            let target = map_lang(&target_lang);

            let body = DeeplRequest {
                text: vec![text],
                target_lang: target,
            };

            let resp = client
                .post(url)
                .header("Authorization", format!("DeepL-Auth-Key {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            let body_text = resp.text().await.unwrap_or_default();

            if !status.is_success() {
                error!("DeeplBackend: 请求失败: {} - {}", status, body_text);

                let msg: String = serde_json::from_str::<Value>(&body_text)
                    .ok()
                    .and_then(|v| v["message"].as_str().map(String::from))
                    .unwrap_or_else(|| body_text.clone());

                return Err(anyhow!("DeepL request failed: {} - {}", status, msg));
            }

            let json: DeeplResponse = serde_json::from_str(&body_text)
                .map_err(|e| anyhow!("DeepL response parse failed: {}", e))?;

            let translated = json
                .translations
                .into_iter()
                .next()
                .map(|t| t.text)
                .ok_or_else(|| {
                    error!("DeeplBackend: 响应缺少 translations");
                    anyhow!("DeepL response missing translations")
                })?;

            debug!("DeeplBackend: 翻译成功");
            Ok(translated)
        })
    }
}
