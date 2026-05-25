use crate::{MetadataParser, USER_AGENT, normalize, text};
use anyhow::{Result, anyhow};
use database::constructors::*;
use log::{debug, error, info};
use models::{Literature, LiteratureType, PublicationType};
use quick_xml::de::from_str;
use reqwest::Client;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Feed {
    #[serde(rename = "entry")]
    entry: Entry,
}

#[derive(Debug, Deserialize)]
struct Entry {
    title: String,
    summary: String,
    published: String,
    #[serde(rename = "author")]
    authors: Vec<ArxivAuthor>,
    #[serde(rename = "id")]
    arxiv_id_url: String,
    #[serde(rename = "journal_ref")]
    journal_ref: Option<String>,
    doi: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArxivAuthor {
    name: String,
}

pub struct ArxivParser;

impl MetadataParser for ArxivParser {
    fn source_id(&self) -> &'static str {
        "ArXiv"
    }

    async fn parse(&self, input: &str) -> Result<Vec<Literature>> {
        let lit = Self::resolve(input).await?;
        Ok(vec![lit])
    }
}

impl ArxivParser {
    pub async fn resolve(arxiv_id: &str) -> Result<Literature> {
        let id = arxiv_id.trim().replace("arXiv:", "");
        info!("解析器: [ArXiv] 正在解析 ID: {id}");

        let url = format!("https://export.arxiv.org/api/query?id_list={id}");
        debug!("解析器: [ArXiv] 请求 URL: {url}");

        let client = Client::new();
        let resp = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?
            .text()
            .await?;

        let feed: Feed = from_str(&resp).map_err(|e| {
            error!("解析器: [ArXiv] XML 解析失败: {e}");
            anyhow!("ArXiv 响应解析失败: {e}")
        })?;
        debug!("解析器: [ArXiv] 成功获取并解析 XML 响应");

        let entry = feed.entry;

        let title = text::clean_title(&entry.title);
        let abstract_text = text::clean_abstract(&entry.summary);
        let raw_id = entry
            .arxiv_id_url
            .split('/')
            .next_back()
            .unwrap_or(&id)
            .to_string();
        // Remove version suffix for cleaner display (e.g., 2301.12345v1 -> 2301.12345)
        let cleaned_arxiv_id = raw_id.split('v').next().unwrap_or(&raw_id).to_string();
        let doi = text::clean_optional_text(entry.doi.as_deref());

        info!("解析器: [ArXiv] 成功获取标题: '{title}'");

        let mut lit =
            create_literature(Uuid::new_v4().to_string(), title, LiteratureType::Preprint);
        lit.arxiv_id = Some(cleaned_arxiv_id);
        lit.doi = doi;
        lit.url = Some(format!("https://arxiv.org/abs/{id}"));

        // 自动规范化标识符 (会清除 arXiv URL 并确保 arxiv_id 格式正确)
        normalize::sanitize_arxiv_identifiers(&mut lit);

        lit.abstract_text = Some(abstract_text);

        // Parse year from published date
        if entry.published.len() >= 4 {
            if let Ok(year) = entry.published[0..4].parse::<i32>() {
                lit.year = Some(year);
                debug!("解析器: [ArXiv] 解析出版年份: {year}");
            }
            // Update created_at to use the actual publishing date if available
            lit.created_at = entry.published.replace('T', " ").replace('Z', "");
        }

        // Try to parse journal_ref for publication info
        if let Some(ref jref) = entry.journal_ref {
            debug!("解析器: [ArXiv] 发现期刊引用信息: {jref}");
            let cleaned_journal = text::clean_publication_name(jref);
            if !cleaned_journal.is_empty() {
                lit.publication = Some(create_publication(
                    cleaned_journal,
                    PublicationType::Journal,
                ));
            }
        }

        let mut author_count = 0;
        for a in entry.authors {
            let cleaned_name = text::clean_author_name(&a.name);
            let parts: Vec<&str> = cleaned_name.split_whitespace().collect();
            if parts.len() >= 2 {
                let first = parts[..parts.len() - 1].join(" ");
                let last = parts.last().unwrap().to_string();
                lit.authors.push(create_author(last, first));
            } else {
                lit.authors.push(create_author(cleaned_name, ""));
            }
            author_count += 1;
        }
        debug!("解析器: [ArXiv] 成功解析出 {author_count} 位作者");

        info!("解析器: [ArXiv] 解析完成，标题: '{}'", lit.title);
        Ok(lit)
    }
}
