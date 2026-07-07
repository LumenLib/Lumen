use anyhow::{Result, anyhow};
use log::{debug, error};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

pub struct YoudaoBackend {
    client: Client,
    appid: String,
    key: String,
    vocab_id: String,
}

#[derive(Deserialize)]
struct YoudaoResponse {
    #[serde(rename = "errorCode")]
    error_code: String,
    translation: Option<Vec<String>>,
}

impl YoudaoBackend {
    pub fn new(secret: &str) -> Self {
        let parts: Vec<&str> = secret.split('#').collect();
        let appid = parts.first().map(|s| s.to_string()).unwrap_or_default();
        let key = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
        let vocab_id = parts.get(2).map(|s| s.to_string()).unwrap_or_default();
        Self {
            client: Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            appid,
            key,
            vocab_id,
        }
    }
}

fn truncate(s: &str) -> String {
    let len = s.chars().count();
    if len <= 20 {
        return s.to_string();
    }
    let first10: String = s.chars().take(10).collect();
    let last10: String = s.chars().skip(len - 10).take(10).collect();
    format!("{}{}{}", first10, len, last10)
}

fn map_lang(lang: &str) -> &str {
    match lang {
        "zh-CN" | "zh-SG" => "zh-CHS",
        "zh-TW" | "zh-HK" | "zh-MO" => "zh-CHT",
        _ => lang.split('-').next().unwrap_or(lang),
    }
}

impl crate::TranslationBackend for YoudaoBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let client = self.client.clone();
        let appid = self.appid.clone();
        let key = self.key.clone();
        let vocab_id = self.vocab_id.clone();
        let text = text.to_string();
        let target_lang = target_lang.to_string();

        Box::pin(async move {
            if appid.is_empty() || key.is_empty() {
                error!("YoudaoBackend: AppID 或 Key 未配置");
                return Err(anyhow!("Youdao AppID or Key is not configured"));
            }

            debug!(
                "YoudaoBackend: 开始翻译, 目标语言={}, 文本长度={}",
                target_lang,
                text.len()
            );

            let salt = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let curtime = (salt / 1000) as u64;

            let sign_input = format!("{}{}{}{}{}", appid, truncate(&text), salt, curtime, key);
            let sign = {
                let mut hasher = Sha256::new();
                hasher.update(sign_input.as_bytes());
                hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>()
            };

            let lang_to = map_lang(&target_lang);

            let mut query = format!(
                "q={}&appKey={}&salt={}&from=auto&to={}&sign={}&signType=v3&curtime={}",
                urlencoding::encode(&text),
                urlencoding::encode(&appid),
                salt,
                urlencoding::encode(lang_to),
                urlencoding::encode(&sign),
                curtime,
            );
            if !vocab_id.is_empty() {
                query.push_str(&format!("&vocabId={}", urlencoding::encode(&vocab_id)));
            }

            let resp = client
                .get(format!("https://openapi.youdao.com/api?{}", query))
                .send()
                .await?;

            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();

            if !status.is_success() {
                error!("YoudaoBackend: 请求失败: {} - {}", status, body);
                return Err(anyhow!("Youdao request failed: {} - {}", status, body));
            }

            let json: YoudaoResponse = serde_json::from_str(&body)
                .map_err(|e| anyhow!("Youdao response parse failed: {}", e))?;

            if json.error_code != "0" {
                error!("YoudaoBackend: API 错误: {}", json.error_code);
                return Err(anyhow!("Youdao API error: {}", json.error_code));
            }

            let translated = json.translation.map(|t| t.concat()).ok_or_else(|| {
                error!("YoudaoBackend: 响应缺少 translation");
                anyhow!("Youdao response missing translation")
            })?;

            debug!("YoudaoBackend: 翻译成功");
            Ok(translated)
        })
    }
}
