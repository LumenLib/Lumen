use crate::{MetadataParser, USER_AGENT, normalize, text};
use anyhow::{Result, anyhow};
use log::{debug, error, info};
use models::constructors::*;
use models::{Literature, LiteratureType, PublicationType};
use reqwest::Client;
use serde_json::Value;
use urlencoding::encode;
use uuid::Uuid;

pub struct DblpParser;

impl MetadataParser for DblpParser {
    fn source_id(&self) -> &'static str {
        "DBLP"
    }

    async fn parse(&self, input: &str) -> Result<Vec<Literature>> {
        Self::search(input).await
    }
}

impl DblpParser {
    /// 通过 DBLP API 搜索文献
    pub async fn search(query: &str) -> Result<Vec<Literature>> {
        info!("解析器: [DBLP] 正在搜索: '{query}'");
        let client = Client::new();
        let url = format!(
            "https://dblp.org/search/publ/api?q={}&format=json&h=20",
            encode(query)
        );
        debug!("解析器: [DBLP] 请求 URL: {url}");

        let response = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response
                .text()
                .await
                .unwrap_or_else(|_| "无法读取响应体".to_string());
            error!(
                "解析器: [DBLP] 搜索失败，状态码: {}, 错误响应内容 (前1000字): {}",
                status,
                err_body.chars().take(1000).collect::<String>()
            );
            return Err(anyhow!("DBLP 搜索失败，状态码: {}", status));
        }

        let raw = response.text().await?;
        debug!(
            "解析器: [DBLP] 原始响应 body (前500字): {}",
            raw.chars().take(500).collect::<String>()
        );
        let json: Value = serde_json::from_str(&raw)?;
        debug!("解析器: [DBLP] 成功解析响应 JSON");
        let hits = json["result"]["hits"]["hit"].as_array();

        let mut results = Vec::new();

        if let Some(hits) = hits {
            info!("解析器: [DBLP] 获取到 {} 条候选结果", hits.len());
            for hit in hits {
                if let Some(info) = hit.get("info") {
                    let title = text::clean_title(info["title"].as_str().unwrap_or("Untitled"));
                    let venue = text::clean_publication_name(info["venue"].as_str().unwrap_or(""));
                    let year_str = text::clean_for_ui_display(info["year"].as_str().unwrap_or(""));
                    let year = year_str.parse::<i32>().ok();
                    let doi = text::clean_optional_text(info["doi"].as_str());
                    let url = text::clean_optional_text(info["ee"].as_str());

                    let lit_type = match info["type"].as_str().unwrap_or("") {
                        "Conference and Workshop Papers" => LiteratureType::Conference,
                        "Journal Articles" => LiteratureType::Article,
                        "Books and Theses" => LiteratureType::Book,
                        _ => LiteratureType::Article,
                    };

                    let mut lit = create_literature(Uuid::new_v4().to_string(), title, lit_type);
                    lit.year = year;
                    lit.doi = doi;
                    lit.url = url;

                    // 自动规范化标识符
                    normalize::sanitize_arxiv_identifiers(&mut lit);

                    lit.volume = text::clean_optional_text(info["volume"].as_str());
                    lit.issue = text::clean_optional_text(info["number"].as_str());
                    lit.pages = text::clean_optional_page_range(info["pages"].as_str());

                    if let Some(y) = year {
                        lit.created_at = crate::time::parse_time_to_ts(&format!("{y}-01-01 00:00:00"));
                    }

                    // 设置出版源
                    if !venue.is_empty() {
                        let pub_type = if lit.literature_type == LiteratureType::Conference {
                            PublicationType::Conference
                        } else {
                            PublicationType::Journal
                        };
                        lit.publication = Some(create_publication(venue, pub_type));
                    }

                    // Authors
                    let mut author_count = 0;
                    if let Some(authors_node) = info.get("authors")
                        && let Some(author_data) = authors_node.get("author")
                    {
                        if let Some(author_array) = author_data.as_array() {
                            for a in author_array {
                                if let Some(name) = a["text"].as_str() {
                                    Self::add_author_from_name(&mut lit, name);
                                    author_count += 1;
                                }
                            }
                        } else if let Some(a) = author_data.as_object()
                            && let Some(name) = a.get("text").and_then(|v| v.as_str())
                        {
                            Self::add_author_from_name(&mut lit, name);
                            author_count += 1;
                        }
                    }
                    debug!(
                        "解析器: [DBLP] 解析条目 '{}' 成功，包含 {} 位作者",
                        lit.title, author_count
                    );

                    results.push(lit);
                }
            }
        }

        info!(
            "解析器: [DBLP] 解析完成，成功转换出 {} 条结构化文献数据",
            results.len()
        );
        Ok(results)
    }

    fn add_author_from_name(lit: &mut Literature, name: &str) {
        // Remove numeric suffixes like "John Smith 0001"
        let name = name.trim();
        let name_without_suffix = if name
            .split_whitespace()
            .last()
            .is_some_and(|last| last.chars().all(|c| c.is_ascii_digit()))
        {
            let parts: Vec<&str> = name.split_whitespace().collect();
            parts[..parts.len() - 1].join(" ")
        } else {
            name.to_string()
        };

        let parts: Vec<&str> = name_without_suffix.split_whitespace().collect();
        if parts.len() >= 2 {
            let first = parts[..parts.len() - 1].join(" ");
            let last = parts.last().unwrap().to_string();
            lit.authors.push(create_author(last, first));
        } else {
            lit.authors.push(create_author(name_without_suffix, ""));
        }
    }
}
