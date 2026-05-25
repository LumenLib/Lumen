use crate::{MetadataParser, USER_AGENT, normalize, text};
use anyhow::{Result, anyhow};
use database::constructors::*;
use log::{debug, error, info};
use models::{Literature, LiteratureType, PublicationType};
use reqwest::Client;
use serde_json::Value;
use urlencoding::encode;

pub struct OpenAlexParser;

impl MetadataParser for OpenAlexParser {
    fn source_id(&self) -> &'static str {
        "OpenAlex"
    }

    async fn parse(&self, input: &str) -> Result<Vec<Literature>> {
        // Default limit to 20 for general search via trait interface
        Self::search(input, 20).await
    }
}

impl OpenAlexParser {
    /// Search works by query string
    pub async fn search(query: &str, limit: usize) -> Result<Vec<Literature>> {
        info!("解析器: [OpenAlex] 正在通过标题搜索文献: '{query}' (限制 {limit} 条)");
        let client = Client::new();
        let safe_query = encode(query);
        let url = format!(
            "https://api.openalex.org/works?search={}&per_page={}",
            safe_query,
            limit.min(100)
        );
        debug!("解析器: [OpenAlex] 请求 URL: {url}");

        let response = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            error!("解析器: [OpenAlex] 搜索失败，状态码: {}", response.status());
            return Err(anyhow!(
                "OpenAlex search failed with status: {}",
                response.status()
            ));
        }

        let raw = response.text().await?;
        debug!("解析器: [OpenAlex] 搜索原始响应 body (前500字): {}", &raw.chars().take(500).collect::<String>());
        let json: Value = serde_json::from_str(&raw)?;
        debug!("解析器: [OpenAlex] 成功解析搜索响应 JSON");
        let results = json["results"].as_array().ok_or_else(|| {
            error!("解析器: [OpenAlex] 响应格式无效，找不到 results 数组");
            anyhow!("Invalid OpenAlex response format")
        })?;

        info!(
            "解析器: [OpenAlex] 搜索成功，获取到 {} 条候选结果",
            results.len()
        );
        let mut literatures = Vec::new();
        for work in results {
            if let Some(lit) = Self::parse_work(work) {
                literatures.push(lit);
            }
        }

        info!(
            "解析器: [OpenAlex] 解析完成，成功转换出 {} 条结构化文献数据",
            literatures.len()
        );
        Ok(literatures)
    }

    /// Resolve a DOI using `OpenAlex`
    pub async fn resolve(doi: &str) -> Result<Literature> {
        let clean_doi = doi.trim();
        info!("解析器: [OpenAlex] 正在通过 DOI 精准匹配文献: {clean_doi}");
        let encoded_doi = encode(clean_doi);
        let url = format!("https://api.openalex.org/works/https://doi.org/{encoded_doi}");
        debug!("解析器: [OpenAlex] 精准匹配 URL: {url}");

        let client = Client::new();
        let response = client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await?;

        if !response.status().is_success() {
            error!(
                "解析器: [OpenAlex] DOI 匹配失败，状态码: {}",
                response.status()
            );
            return Err(anyhow!(
                "OpenAlex DOI lookup failed with status: {}",
                response.status()
            ));
        }

        let raw = response.text().await?;
        debug!("解析器: [OpenAlex] DOI 匹配原始响应 body (前500字): {}", &raw.chars().take(500).collect::<String>());
        let json: Value = serde_json::from_str(&raw)?;
        debug!("解析器: [OpenAlex] 成功解析精准匹配响应 JSON");
        let result = Self::parse_work(&json).ok_or_else(|| {
            error!("解析器: [OpenAlex] 解析响应数据失败，未找到文献信息");
            anyhow!("Literature not found in OpenAlex")
        });

        if let Ok(ref lit) = result {
            info!("解析器: [OpenAlex] 匹配成功，标题: '{}'", lit.title);
        }
        result
    }

    /// Parse `OpenAlex` work JSON to Literature
    fn parse_work(work: &Value) -> Option<Literature> {
        let title_val = work["title"].as_str();
        if title_val.is_none() {
            debug!("解析器: [OpenAlex] 跳过缺失标题的条目");
            return None;
        }
        let title = text::clean_title(title_val?);
        if title.is_empty() {
            debug!("解析器: [OpenAlex] 跳过标题为空的条目");
            return None;
        }

        let type_val = work["type"].as_str();
        if type_val.is_none() {
            debug!("解析器: [OpenAlex] 跳过缺失类型的条目: '{title}'");
            return None;
        }
        let lit_type = Self::map_type(type_val?).clone();

        let mut lit = create_literature(
            uuid::Uuid::new_v4().to_string(),
            title.clone(),
            lit_type.clone(),
        );

        // DOI
        if let Some(doi) = work["doi"].as_str() {
            let doi_val = doi.trim().replace("https://doi.org/", "");
            lit.doi = Some(doi_val);
            normalize::sanitize_arxiv_identifiers(&mut lit);

            if lit.arxiv_id.is_some() {
                debug!(
                    "解析器: [OpenAlex] 从 DOI 中提取出 ArXiv ID: {:?}",
                    lit.arxiv_id
                );
            }
        }

        // Authors
        let mut author_count = 0;
        if let Some(authors) = work["authorships"].as_array() {
            for authorship in authors {
                if let Some(author) = authorship.get("author").and_then(|a| a.as_object())
                    && let Some(name) = author.get("display_name").and_then(|n| n.as_str())
                {
                    let parts: Vec<&str> = name.splitn(2, ' ').collect();
                    if parts.len() >= 2 {
                        lit.authors.push(create_author(
                            text::clean_author_name(parts[1]),
                            text::clean_author_name(parts[0]),
                        ));
                    } else {
                        lit.authors
                            .push(create_author(text::clean_author_name(name), String::new()));
                    }
                    author_count += 1;
                }
            }
        }
        debug!("解析器: [OpenAlex] 解析文献 '{title}' 成功，包含 {author_count} 位作者");

        // Publication year
        if let Some(year) = work["publication_year"].as_i64() {
            lit.year = Some(year as i32);
        }

        // Journal/Conference info from primary_location
        if let Some(location) = work["primary_location"].as_object() {
            if let Some(source) = location.get("source").and_then(|s| s.as_object()) {
                if let Some(journal_name) = source.get("display_name").and_then(|n| n.as_str()) {
                    let cleaned_name = text::clean_publication_name(journal_name);
                    if !cleaned_name.is_empty() {
                        let pub_type = if lit_type == LiteratureType::Conference {
                            PublicationType::Conference
                        } else {
                            PublicationType::Journal
                        };
                        lit.publication = Some(create_publication(cleaned_name, pub_type));
                    }
                }

                // Bibliographic info
                if let Some(biblio) = location.get("biblio").and_then(|b| b.as_object()) {
                    lit.volume = biblio
                        .get("volume")
                        .and_then(|v| v.as_str())
                        .map(text::clean_for_ui_display)
                        .filter(|s| !s.is_empty());
                    lit.issue = biblio
                        .get("issue")
                        .and_then(|i| i.as_str())
                        .map(text::clean_for_ui_display)
                        .filter(|s| !s.is_empty());
                    lit.pages = Self::format_pages(
                        biblio.get("first_page").and_then(|p| p.as_str()),
                        biblio.get("last_page").and_then(|p| p.as_str()),
                    )
                    .map(|p| text::clean_page_range(&p)); // 确保 OpenAlex 返回的页码也被规范化
                }
            }

            // Landing page URL
            if let Some(url) = location.get("landing_page_url").and_then(|u| u.as_str()) {
                let cleaned_url = text::clean_for_ui_display(url);
                if !cleaned_url.is_empty() {
                    lit.url = Some(cleaned_url);
                    normalize::sanitize_arxiv_identifiers(&mut lit);
                }
            }
        }

        // Abstract (reconstructed from inverted index)
        if let Some(abstract_inv) = work["abstract_inverted_index"].as_object()
            && !abstract_inv.is_empty()
        {
            let mut word_pos = Vec::new();
            for (word, positions) in abstract_inv {
                if let Some(pos_array) = positions.as_array() {
                    for pos in pos_array {
                        if let Some(p) = pos.as_u64() {
                            word_pos.push((p, word));
                        }
                    }
                }
            }
            word_pos.sort_by_key(|k| k.0);
            let reconstructed: Vec<_> = word_pos.into_iter().map(|(_, w)| w.as_str()).collect();
            lit.abstract_text = Some(text::clean_abstract(&reconstructed.join(" ")));
        }

        // Publisher - set on publication object
        if let Some(publisher) = work["publisher"].as_str() {
            let publisher_str = text::clean_for_ui_display(publisher);
            if !publisher_str.is_empty() {
                if let Some(ref mut pub_obj) = lit.publication {
                    pub_obj.publisher = Some(publisher_str);
                } else {
                    // Create a new Publication if none exists
                    let pub_type = if lit_type == LiteratureType::Conference {
                        PublicationType::Conference
                    } else {
                        PublicationType::Journal
                    };
                    let mut new_pub = create_publication(String::new(), pub_type);
                    new_pub.publisher = Some(publisher_str);
                    lit.publication = Some(new_pub);
                }
            }
        }

        // Open Access status
        if let Some(oa) = work["open_access"].as_object()
            && oa
                .get("is_oa")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
            && let Some(oa_url) = oa.get("oa_url").and_then(|u| u.as_str())
        {
            // Store as URL field for now
            let cleaned_oa_url = text::clean_for_ui_display(oa_url);
            if lit.url.is_none() {
                lit.url = Some(cleaned_oa_url);
                normalize::sanitize_arxiv_identifiers(&mut lit);
            }
        }

        // Citation count
        lit.keywords = work["keywords"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| text::clean_optional_text(v["display_name"].as_str()))
                    .collect()
            })
            .unwrap_or_default();

        Some(lit)
    }

    /// Map `OpenAlex` publication type to `LiteratureType`
    fn map_type(openalex_type: &str) -> LiteratureType {
        match openalex_type {
            "journal-article" => LiteratureType::Article,
            "book-chapter" | "book-section" => LiteratureType::Book,
            "book" => LiteratureType::Book,
            "proceedings-article" | "conference-paper" => LiteratureType::Conference,
            "posted-content" => LiteratureType::Preprint,
            "thesis" => LiteratureType::Thesis,
            "report" => LiteratureType::TechnicalReport,
            "webpage" => LiteratureType::Webpage,
            _ => LiteratureType::Article,
        }
    }

    /// Format page range
    fn format_pages(first: Option<&str>, last: Option<&str>) -> Option<String> {
        match (first, last) {
            (Some(f), Some(l)) => Some(format!("{f}-{l}")),
            (Some(f), None) => Some(f.to_string()),
            _ => None,
        }
    }
}
