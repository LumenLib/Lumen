use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct NiuTransBackend {
    client: Client,
    api_key: String,
}

#[derive(Serialize)]
struct NiuTransRequest<'a> {
    from: &'a str,
    to: &'a str,
    apikey: &'a str,
    src_text: &'a str,
}

#[derive(Deserialize)]
struct NiuTransResponse {
    #[serde(rename = "tgt_text")]
    tgt_text: Option<String>,
    error_msg: Option<String>,
}

impl NiuTransBackend {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            api_key,
        }
    }
}

impl crate::TranslationBackend for NiuTransBackend {
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
                error!("NiuTransBackend: API Key 未配置");
                return Err(anyhow!("NiuTrans API Key is not configured"));
            }

            debug!(
                "NiuTransBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );
            let lang_to = if target_lang.starts_with("zh") {
                "zh"
            } else {
                &target_lang
            };

            let url = "https://api.niutrans.com/NiuTransServer/translation";

            let body = NiuTransRequest {
                from: "auto",
                to: lang_to,
                apikey: &api_key,
                src_text: &text,
            };

            let resp = client.post(url).form(&body).send().await?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                error!("NiuTransBackend: 请求失败: {} - {}", status, body);
                return Err(anyhow!("NiuTrans request failed: {} - {}", status, body));
            }

            let json: NiuTransResponse = resp.json().await?;
            if let Some(translated) = json.tgt_text {
                debug!("NiuTransBackend: 翻译成功");
                Ok(translated)
            } else {
                let msg = json
                    .error_msg
                    .unwrap_or_else(|| "Unknown error".to_string());
                error!("NiuTransBackend: 翻译失败: {}", msg);
                Err(anyhow!("NiuTrans error: {}", msg))
            }
        })
    }
}
