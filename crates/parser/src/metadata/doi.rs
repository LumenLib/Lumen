use crate::{MetadataParser, USER_AGENT, normalize, text};
use anyhow::{Result, anyhow};
use database::constructors::*;
use log::{debug, error, info};
use models::{Literature, LiteratureType, PublicationType};
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use uuid::Uuid;

pub struct DoiParser;

impl MetadataParser for DoiParser {
    fn source_id(&self) -> &'static str {
        "DOI"
    }

    async fn parse(&self, input: &str) -> Result<Vec<Literature>> {
        let lit = Self::resolve(input).await?;
        Ok(vec![lit])
    }
}

impl DoiParser {
    /// 通过 Crossref API 获取 DOI 信息
    pub async fn resolve(doi: &str) -> Result<Literature> {
        let doi = doi.trim();
        info!("解析器: [Crossref] 正在解析 DOI: {doi}");
        let client = Client::new();
        let url = format!("https://api.crossref.org/works/{doi}");
        debug!("解析器: [Crossref] 请求 URL: {url}");

        let response = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            error!("解析器: [Crossref] 解析失败，状态码: {}", response.status());
            return Err(anyhow!("DOI 解析失败，状态码: {}", response.status()));
        }

        let raw = response.text().await?;
        debug!(
            "解析器: [Crossref] 原始响应 body (前500字): {}",
            raw.chars().take(500).collect::<String>()
        );
        let json: Value = serde_json::from_str(&raw)?;
        debug!("解析器: [Crossref] 成功解析响应 JSON");
        let work = json.get("message").ok_or_else(|| {
            error!("解析器: [Crossref] 响应格式无效，找不到 message 节点");
            anyhow!("无效的 API响应")
        })?;

        let title = text::clean_title(work["title"][0].as_str().unwrap_or("Untitled"));
        info!("解析器: [Crossref] 解析成功，标题: '{title}'");

        let lit_type = match work["type"].as_str().unwrap_or("") {
            "journal-article" => LiteratureType::Article,
            "book" => LiteratureType::Book,
            "proceedings-article" => LiteratureType::Conference,
            _ => LiteratureType::Article,
        };

        let mut lit = create_literature(Uuid::new_v4().to_string(), title, lit_type);
        lit.doi = Some(doi.to_string());
        normalize::sanitize_arxiv_identifiers(&mut lit);

        if lit.arxiv_id.is_some() {
            debug!(
                "解析器: [Crossref] 从 DOI 中提取出 ArXiv ID: {:?}",
                lit.arxiv_id
            );
        }

        // Authors
        let mut author_count = 0;
        if let Some(authors) = work["author"].as_array() {
            for a in authors {
                let family = text::clean_author_name(a["family"].as_str().unwrap_or(""));
                let given = text::clean_author_name(a["given"].as_str().unwrap_or(""));
                lit.authors.push(create_author(family, given));
                author_count += 1;
            }
        }
        debug!("解析器: [Crossref] 解析出 {author_count} 位作者");

        // Year, Month and Day
        if let Some(date_parts) = work["published-print"]["date-parts"]
            .as_array()
            .or_else(|| work["published-online"]["date-parts"].as_array())
            && let Some(parts) = date_parts.first().and_then(|v| v.as_array())
        {
            if let Some(year) = parts.first().and_then(|v| v.as_i64()) {
                lit.year = Some(year as i32);
                debug!("解析器: [Crossref] 解析出年份: {year}");
            }
            if let Some(month) = parts.get(1).and_then(|v| v.as_i64()) {
                lit.month = Some(month as i32);
                debug!("解析器: [Crossref] 解析出月份: {month}");
            }
            if let Some(day) = parts.get(2).and_then(|v| v.as_i64()) {
                lit.day = Some(day as i32);
                debug!("解析器: [Crossref] 解析出日: {day}");
            }
        }

        // Publication (Journal/Conference)
        if let Some(container_title) = work["container-title"].as_array()
            && !container_title.is_empty()
        {
            let name = text::clean_publication_name(container_title[0].as_str().unwrap_or(""));
            if !name.is_empty() {
                let pub_type = if lit.literature_type == LiteratureType::Conference {
                    PublicationType::Conference
                } else {
                    PublicationType::Journal
                };
                lit.publication = Some(create_publication(name, pub_type));
            }
        }

        // Set publisher on publication object
        if let Some(publisher_str) = text::clean_optional_text(work["publisher"].as_str()) {
            if let Some(ref mut pub_obj) = lit.publication {
                pub_obj.publisher = Some(publisher_str);
            } else {
                // Create a new Publication if none exists
                let pub_type = if lit.literature_type == LiteratureType::Conference {
                    PublicationType::Conference
                } else {
                    PublicationType::Journal
                };
                let mut new_pub = create_publication(String::new(), pub_type);
                new_pub.publisher = Some(publisher_str);
                lit.publication = Some(new_pub);
            }
        }
        lit.volume = text::clean_optional_text(work["volume"].as_str());
        lit.issue = text::clean_optional_text(work["issue"].as_str());
        lit.pages = text::clean_optional_page_range(work["page"].as_str());

        if let Some(abs) = work["abstract"].as_str() {
            lit.abstract_text = clean_crossref_abstract(Some(abs));
            if let Some(abstract_text) = &lit.abstract_text {
                debug!(
                    "解析器: [Crossref] 成功解析并清理摘要内容 (长度: {} 字符)",
                    abstract_text.len()
                );
            }
        }

        Ok(lit)
    }
}

/// 清理 Crossref API 返回的摘要文本
///
/// Crossref 摘要通常包含 JATS (Journal Article Tag Suite) XML 格式，
/// 例如: <jats:title>Abstract</jats:title><jats:p>文本内容...</jats:p>
///
/// 处理逻辑:
/// 1. 首先尝试提取所有 `<jats:p>` 标签内的内容（这是真正的摘要内容）
/// 2. 如果找不到 `<jats:p>` 标签，尝试普通的 `<p>` 标签
/// 3. 如果都没有，则移除所有 XML/HTML 标签作为后备方案
///
/// 注意: `<jats:title>` 标签中的 "Abstract" 是标题，不是摘要内容的一部分，
/// 因此会被忽略，只保留 `<jats:p>` 标签内的正文。
fn clean_crossref_abstract(abstract_text: Option<&str>) -> Option<String> {
    let text = abstract_text?;

    if text.trim().is_empty() {
        return None;
    }

    // 如果提取到了 <jats:p> 标签内容，处理它们

    // 第一步: 尝试提取 <jats:p> 标签内容（这是 Crossref 的标准格式）
    // 使用非贪婪匹配 (?s) 允许点号匹配换行符
    let re_paragraph = Regex::new(r"(?s)<jats:p>(.*?)</jats:p>").unwrap_or_else(|_| {
        // 如果正则表达式编译失败，使用简单的标签移除作为后备
        Regex::new(r"<[^>]+>").unwrap()
    });

    let mut paragraphs = Vec::new();

    for capture in re_paragraph.captures_iter(text) {
        if let Some(content) = capture.get(1) {
            let paragraph = content.as_str().trim();
            if !paragraph.is_empty() {
                paragraphs.push(paragraph);
            }
        }
    }

    // 第二步: 如果没有找到 jats:p 标签，尝试普通的 <p> 标签（某些API可能使用这种格式）
    if paragraphs.is_empty() {
        let re_other_p =
            Regex::new(r"(?s)<p>(.*?)</p>").unwrap_or_else(|_| Regex::new(r"<[^>]+>").unwrap());

        for capture in re_other_p.captures_iter(text) {
            if let Some(content) = capture.get(1) {
                let paragraph = content.as_str().trim();
                if !paragraph.is_empty() {
                    paragraphs.push(paragraph);
                }
            }
        }
    }

    // 第三步: 作为最后的手段，如果以上方法都没有提取到内容，
    // 移除所有标签，保留所有文本内容（包括可能误包含的"Abstract"字样）
    if paragraphs.is_empty() {
        let re_tags = Regex::new(r"<[^>]+>").unwrap_or_else(|_| Regex::new(r".").unwrap());
        let without_tags = re_tags.replace_all(text, "").to_string();
        let cleaned = text::clean_abstract(&without_tags);
        if cleaned.trim().is_empty() {
            return None;
        }
        return Some(cleaned);
    }

    // 第四步: 合并所有提取到的段落，用空格分隔，然后进行文本清理
    let combined = paragraphs.join(" ");
    let cleaned = text::clean_abstract(&combined);

    if cleaned.trim().is_empty() {
        None
    } else {
        Some(cleaned)
    }
}
