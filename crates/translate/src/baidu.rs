use anyhow::{Result, anyhow};
use log::{debug, error};
use md5::{Digest, Md5};
use reqwest::Client;
use serde::Deserialize;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct BaiduBackend {
    client: Client,
    appid: String,
    key: String,
    action: String,
}

#[derive(Deserialize)]
struct BaiduTransResult {
    dst: String,
}

#[derive(Deserialize)]
struct BaiduResponse {
    #[serde(rename = "error_code")]
    error_code: Option<String>,
    #[serde(rename = "error_msg")]
    error_msg: Option<String>,
    trans_result: Option<Vec<BaiduTransResult>>,
}

impl BaiduBackend {
    pub fn new(secret: &str) -> Self {
        let parts: Vec<&str> = secret.split('#').collect();
        let appid = parts.first().map(|s| s.to_string()).unwrap_or_default();
        let key = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
        let action = parts
            .get(2)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "0".to_string());
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            appid,
            key,
            action,
        }
    }
}

impl crate::TranslationBackend for BaiduBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let client = self.client.clone();
        let appid = self.appid.clone();
        let key = self.key.clone();
        let action = self.action.clone();
        let text = text.to_string();
        let target_lang = target_lang.to_string();

        Box::pin(async move {
            if appid.is_empty() || key.is_empty() {
                error!("BaiduBackend: AppID 或 Key 未配置");
                return Err(anyhow!("Baidu AppID or Key is not configured"));
            }

            debug!(
                "BaiduBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );

            let salt = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();

            let sign_input = format!("{}{}{}{}", appid, text, salt, key);
            let sign = {
                let mut hasher = Md5::new();
                hasher.update(sign_input.as_bytes());
                hasher
                    .finalize()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>()
            };

            let lang_to = target_lang.split('-').next().unwrap_or(&target_lang);

            let url = "https://fanyi-api.baidu.com/api/trans/vip/translate";
            let params = [
                ("q", text.as_str()),
                ("from", "auto"),
                ("to", lang_to),
                ("appid", appid.as_str()),
                ("salt", &salt.to_string()),
                ("sign", &sign),
                ("action", &action),
            ];

            let resp = client.post(url).form(&params).send().await?;

            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if !status.is_success() {
                error!("BaiduBackend: 请求失败: {} - {}", status, body);
                return Err(anyhow!("Baidu request failed: {} - {}", status, body));
            }

            let json: BaiduResponse = serde_json::from_str(&body)
                .map_err(|e| anyhow!("Baidu response parse failed: {}", e))?;

            if let Some(code) = json.error_code {
                let msg = json.error_msg.unwrap_or_default();
                error!("BaiduBackend: API 错误: {} - {}", code, msg);
                return Err(anyhow!("Baidu API error: {} - {}", code, msg));
            }

            let translated = json
                .trans_result
                .map(|r| r.into_iter().map(|t| t.dst).collect::<String>())
                .ok_or_else(|| {
                    error!("BaiduBackend: 响应缺少 trans_result");
                    anyhow!("Baidu response missing trans_result")
                })?;

            debug!("BaiduBackend: 翻译成功");
            Ok(translated)
        })
    }
}
