use anyhow::{Result, anyhow};
use log::{debug, error, info};
use std::future::Future;
use std::pin::Pin;

use crate::TranslationBackend;

pub struct AiTranslateBackend {
    service: ai::AiService,
    target_lang_map: Vec<(String, String)>,
}

impl AiTranslateBackend {
    pub fn new(kind: ai::BackendKind, config: &ai::AiConfig) -> Self {
        info!(
            "AiTranslateBackend::new: kind={:?}, model={}, api_base={}",
            kind, config.model, config.api_base,
        );
        Self {
            service: ai::AiService::new(kind, config),
            target_lang_map: vec![
                ("zh".to_string(), "中文".to_string()),
                ("zh-CN".to_string(), "简体中文".to_string()),
                ("zh-TW".to_string(), "繁体中文".to_string()),
                ("en".to_string(), "英语".to_string()),
                ("ja".to_string(), "日语".to_string()),
                ("ko".to_string(), "韩语".to_string()),
                ("fr".to_string(), "法语".to_string()),
                ("de".to_string(), "德语".to_string()),
                ("es".to_string(), "西班牙语".to_string()),
                ("pt".to_string(), "葡萄牙语".to_string()),
                ("ru".to_string(), "俄语".to_string()),
                ("ar".to_string(), "阿拉伯语".to_string()),
            ],
        }
    }

    fn map_lang(&self, code: &str) -> String {
        let code = code.split('-').next().unwrap_or(code);
        self.target_lang_map
            .iter()
            .find(|(c, _)| c == code)
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| {
                debug!(
                    "AiTranslateBackend: 未找到语言映射, code={}, 直接使用原始代码",
                    code
                );
                code.to_string()
            })
    }
}

impl TranslationBackend for AiTranslateBackend {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>> {
        let service = self.service.clone();
        let text_len = text.len();
        let text = text.to_string();
        let target_lang = target_lang.to_string();
        let lang_name = self.map_lang(&target_lang);

        info!(
            "AiTranslateBackend::translate: 开始, backend={}, model={}, lang={}, lang_name={}, text_len={}",
            service.name(),
            service.model(),
            target_lang,
            lang_name,
            text_len,
        );

        Box::pin(async move {
            if text.is_empty() {
                debug!("AiTranslateBackend::translate: 文本为空，直接返回");
                return Ok(String::new());
            }

            debug!(
                "AiTranslateBackend::translate: 构造 prompt, target={lang_name}, text_preview={}",
                &text,
            );

            let messages = vec![ai::ChatMessage::user(format!(
                "将以下学术文本翻译为{lang_name}，保持学术风格和专业术语准确性，只返回翻译结果：\n\n{text}"
            ))];

            let system_prompt = "你是一个学术翻译助手。必须遵守以下规则：\n1. 保持原意、学术风格和术语准确性；\n2. 只返回翻译结果，不要添加任何解释或额外内容；\n3. 禁止使用 'Here is the translation'、'翻译如下' 等引导语；\n4. 不要重复原文，直接输出翻译。";

            debug!("AiTranslateBackend::translate: 调用 AiService::chat...");
            match service.chat(&messages, Some(system_prompt)).await {
                Ok(result) => {
                    let result = result.trim().to_string();
                    info!(
                        "AiTranslateBackend::translate: 成功, result_len={}, result_preview={}",
                        result.len(),
                        &result,
                    );
                    Ok(result)
                }
                Err(e) => {
                    error!("AiTranslateBackend::translate: 翻译失败: {e:?}");
                    Err(anyhow!("AI 翻译失败: {e}"))
                }
            }
        })
    }
}
