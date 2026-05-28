use anyhow::{Result, anyhow};
use log::{debug, error, info};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub mod baidu;
pub mod bing_free;
pub mod deepl;
pub mod google;
pub mod google_free;
pub mod niutrans;
pub mod youdao;

pub trait TranslationBackend: Send + Sync {
    fn translate(
        &self,
        text: &str,
        target_lang: &str,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;
}

pub type BackendConstructor = fn(&HashMap<String, String>) -> Arc<dyn TranslationBackend>;

pub struct EngineInfo {
    pub id: &'static str,
    pub limit: usize,
    pub requires_keys: &'static [&'static str],
    pub is_free: bool,
    pub construct: BackendConstructor,
}

fn construct_google_free(_keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(google_free::GoogleFreeBackend::new())
}

fn construct_bing_free(_keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(bing_free::BingFreeBackend::new())
}

fn construct_google(keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(google::GoogleBackend::new(
        keys.get("google").cloned().unwrap_or_default(),
    ))
}

fn construct_niutrans(keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(niutrans::NiuTransBackend::new(
        keys.get("niutrans").cloned().unwrap_or_default(),
    ))
}

fn construct_baidu(keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(baidu::BaiduBackend::new(
        keys.get("baidu").cloned().unwrap_or_default().as_str(),
    ))
}

fn construct_youdao(keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(youdao::YoudaoBackend::new(
        keys.get("youdao").cloned().unwrap_or_default().as_str(),
    ))
}

fn construct_deepl_free(keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(deepl::DeeplBackend::new(
        keys.get("deepl").cloned().unwrap_or_default(),
        false,
    ))
}

fn construct_deepl_pro(keys: &HashMap<String, String>) -> Arc<dyn TranslationBackend> {
    Arc::new(deepl::DeeplBackend::new(
        keys.get("deepl").cloned().unwrap_or_default(),
        true,
    ))
}

pub static ENGINES: &[EngineInfo] = &[
    EngineInfo {
        id: "google_free",
        limit: 5000,
        requires_keys: &[],
        is_free: true,
        construct: construct_google_free,
    },
    EngineInfo {
        id: "bing_free",
        limit: 1000,
        requires_keys: &[],
        is_free: true,
        construct: construct_bing_free,
    },
    EngineInfo {
        id: "google",
        limit: 0,
        requires_keys: &["google"],
        is_free: false,
        construct: construct_google,
    },
    EngineInfo {
        id: "niutrans",
        limit: 0,
        requires_keys: &["niutrans"],
        is_free: false,
        construct: construct_niutrans,
    },
    EngineInfo {
        id: "baidu",
        limit: 6000,
        requires_keys: &["baidu"],
        is_free: false,
        construct: construct_baidu,
    },
    EngineInfo {
        id: "youdao",
        limit: 0,
        requires_keys: &["youdao"],
        is_free: false,
        construct: construct_youdao,
    },
    EngineInfo {
        id: "deepl_free",
        limit: 0,
        requires_keys: &["deepl"],
        is_free: true,
        construct: construct_deepl_free,
    },
    EngineInfo {
        id: "deepl_pro",
        limit: 0,
        requires_keys: &["deepl"],
        is_free: false,
        construct: construct_deepl_pro,
    },
];

#[derive(Clone)]
pub struct TranslationService {
    backend: Arc<dyn TranslationBackend>,
    cache: Arc<Mutex<HashMap<u64, String>>>,
    limit: usize,
}

impl TranslationService {
    pub fn new(engine: &str, keys: &HashMap<String, String>) -> Self {
        info!("TranslationService: 创建翻译服务, 引擎={}", engine);
        let mut this = Self {
            backend: Arc::new(google_free::GoogleFreeBackend::new()),
            cache: Arc::new(Mutex::new(HashMap::new())),
            limit: 0,
        };
        this.switch_engine(engine, keys);
        this
    }

    pub fn switch_engine(&mut self, engine: &str, keys: &HashMap<String, String>) {
        info!("TranslationService: 切换引擎, 新引擎={}", engine);
        let info = ENGINES
            .iter()
            .find(|e| e.id == engine)
            .unwrap_or_else(|| &ENGINES[0]);
        self.backend = (info.construct)(keys);
        self.limit = info.limit;
        self.cache = Arc::new(Mutex::new(HashMap::new()));
    }

    pub async fn translate(&self, text: &str, target_lang: &str) -> Result<String> {
        if text.is_empty() {
            return Ok(String::new());
        }

        let mut h = DefaultHasher::new();
        text.hash(&mut h);
        target_lang.hash(&mut h);
        let key = h.finish();
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&key) {
                return Ok(cached.clone());
            }
        }

        debug!(
            "TranslationService: 翻译, 目标语言={}, 文本长度={}",
            target_lang,
            text.len()
        );

        let result_cap = text.len();
        let parts = if self.limit > 0 && text.len() > self.limit {
            split_text(text, self.limit)
        } else {
            vec![text.to_string()]
        };

        let mut result = String::with_capacity(result_cap);
        let mut errors: Vec<String> = Vec::new();

        for (i, part) in parts.iter().enumerate() {
            match self.backend.translate(part, target_lang).await {
                Ok(translated) => result.push_str(&translated),
                Err(e) => {
                    error!(
                        "TranslationService: [第{}/{}段翻译失败: {}]",
                        i + 1,
                        parts.len(),
                        e
                    );
                    let msg = format!("\n[第{}段翻译失败]", i + 1);
                    result.push_str(&msg);
                    errors.push(e.to_string());
                }
            }
        }

        if errors.is_empty() {
            debug!("TranslationService: 翻译完成, 结果长度={}", result.len());
            let mut cache = self.cache.lock().unwrap();
            if cache.len() >= 500 {
                cache.clear();
            }
            cache.insert(key, result.clone());
            Ok(result)
        } else if errors.len() < parts.len() {
            debug!(
                "TranslationService: 翻译完成(部分), 失败={}/{}",
                errors.len(),
                parts.len()
            );
            Ok(result)
        } else {
            Err(anyhow!(
                "所有 {} 段翻译均失败: {}",
                parts.len(),
                errors.join("; ")
            ))
        }
    }
}

/// 按 `limit` 字节拆分文本，优先在合适断点处拆分
fn split_text(text: &str, limit: usize) -> Vec<String> {
    if text.len() <= limit {
        return vec![text.to_string()];
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let remaining = &text[start..];
        if remaining.len() <= limit {
            parts.push(remaining.to_string());
            break;
        }
        let end = find_break(remaining, limit);
        parts.push(remaining[..end].to_string());
        start += end;
    }
    parts
}

const ABBREVIATIONS: &[&str] = &[
    "Co", "Corp", "Dept", "Dr", "Eq", "Eqs", "Est", "Fig", "Inc", "Jr", "Ltd", "Mr", "Mrs", "Ms",
    "Prof", "Sec", "Sr", "St", "Vol", "approx", "avg", "ca", "cf", "dept", "e.g", "est", "et al",
    "etc", "i.e", "inc", "max", "min", "ref", "viz", "vs",
];

fn is_abbreviation(s: &str, dot_before: usize) -> bool {
    let before = &s[..dot_before];
    ABBREVIATIONS.iter().any(|abbr| {
        if !before.ends_with(abbr) {
            return false;
        }
        let start = dot_before.checked_sub(abbr.len()).unwrap_or(0);
        start == 0 || !s[..start].chars().last().is_some_and(|c| c.is_alphabetic())
    })
}

/// 在 `lim` 字节内找到最佳断点（从高优到低优查找）
fn find_break(s: &str, lim: usize) -> usize {
    let max = s
        .char_indices()
        .take_while(|(i, _)| *i <= lim)
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    let search = &s[..max];
    for pat in &["\n\n", "\n", "。", "！", "？"] {
        if let Some(pos) = search.rfind(pat) {
            return pos + pat.len();
        }
    }
    if let Some(pos) = search.rfind(". ") {
        if !is_abbreviation(search, pos) {
            return pos + 2;
        }
    }
    for pat in &["! ", "? ", " "] {
        if let Some(pos) = search.rfind(pat) {
            return pos + pat.len();
        }
    }
    max
}
