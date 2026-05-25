use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

pub struct NiuTransBackend {
    client: Client,
    api_key: String,
}

#[derive(Serialize)]
struct NiuTransRequest<'a> {
    from: &'a str,
    to: &'a str,
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
            client: Client::new(),
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

            debug!("NiuTransBackend: 开始翻译, 目标语言={}, 文本长度={}", target_lang, text.len());
            let lang_to = if target_lang.starts_with("zh") {
                "zh"
            } else {
                &target_lang
            };

            let url = format!(
                "https://niutrans.com/niuInterface/textTranslation?apikey={}",
                api_key
            );

            let body = NiuTransRequest {
                from: "auto",
                to: lang_to,
                src_text: &text,
            };

            let resp = client.post(url).json(&body).send().await?;

            if !resp.status().is_success() {
                error!("NiuTransBackend: 请求失败: {}", resp.status());
                return Err(anyhow!("NiuTrans request failed: {}", resp.status()));
            }

            let json: NiuTransResponse = resp.json().await?;
            if let Some(translated) = json.tgt_text {
                debug!("NiuTransBackend: 翻译成功");
                Ok(translated)
            } else {
                let msg = json.error_msg.unwrap_or_else(|| "Unknown error".to_string());
                error!("NiuTransBackend: 翻译失败: {}", msg);
                Err(anyhow!("NiuTrans error: {}", msg))
            }
        })
    }

    fn name(&self) -> &'static str {
        "小牛翻译"
    }
}
