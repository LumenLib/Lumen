use crate::text;
use anyhow::{Result, anyhow};
use database::constructors::*;
use log::{debug, error, info};
use models::{Author, FeedItem};
use quick_xml::{events::Event, reader::Reader};
use uuid::Uuid;

/// 通用学术 RSS 解析器
///
/// 支持标准 RSS 2.0, RSS 1.0 (RDF), Atom
/// 自动识别并提取 Dublin Core (dc:) 和 PRISM (prism:) 元数据
pub struct RssSubscriptionParser;

impl RssSubscriptionParser {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for RssSubscriptionParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RssSubscriptionParser {
    pub fn parse(xml: &str, feed_id: &str) -> Result<(Vec<FeedItem>, Option<String>)> {
        info!("开始使用通用 RSS 解析器解析: {feed_id}");
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut items = Vec::new();
        let mut buf = Vec::new();

        let mut in_item = false;
        let mut current_tag = String::new();

        // 字段缓存
        let mut title = String::new();
        let mut link = String::new();
        let mut description = String::new();
        let mut content_encoded = String::new();

        // 标识符
        let mut guid = String::new();
        let mut dc_identifier = String::new();
        let mut prism_doi = String::new();

        // 日期
        let mut pub_date = String::new(); // RSS 2.0
        let mut dc_date = String::new(); // DC
        let mut prism_date = String::new(); // PRISM

        // 出版信息
        let mut journal = String::new();
        let mut volume = String::new();
        let mut issue = String::new();

        // 作者 (可能由多个标签组成)
        let mut authors_list: Vec<String> = Vec::new();

        let mut channel_update_time = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    // RSS uses <item>, RDF uses <item> (usually flat), Atom uses <entry>
                    if name == "item" || name == "entry" {
                        in_item = true;
                        // 重置字段
                        title.clear();
                        link.clear();
                        description.clear();
                        content_encoded.clear();
                        guid.clear();
                        dc_identifier.clear();
                        prism_doi.clear();
                        pub_date.clear();
                        dc_date.clear();
                        prism_date.clear();
                        journal.clear();
                        volume.clear();
                        issue.clear();
                        authors_list.clear();
                    }
                    current_tag = name;
                }
                Ok(Event::Text(e)) => {
                    let text = String::from_utf8_lossy(e.as_ref()).to_string();
                    if in_item {
                        match current_tag.as_str() {
                            "title" | "dc:title" => {
                                if title.is_empty() || current_tag == "dc:title" {
                                    title = text;
                                }
                            }
                            "link" => link = text,
                            "description" => description = text,
                            "content:encoded" => content_encoded = text,

                            // 标识符处理
                            "guid" => guid = text,
                            "dc:identifier" => dc_identifier = text,
                            "prism:doi" => prism_doi = text,

                            // 日期处理
                            "pubDate" => pub_date = text,
                            "dc:date" => dc_date = text,
                            "prism:coverDate" | "prism:publicationDate" => prism_date = text,

                            // 出版元数据
                            "prism:publicationName" | "dc:source" => journal = text,
                            "prism:volume" => volume = text,
                            "prism:number" | "prism:issue" => issue = text,

                            // 作者处理
                            "dc:creator" | "author" => {
                                // 某些源将所有作者放在一个标签中，某些源使用多个标签
                                // 我们先收集所有文本，后续在 End 标签处统一处理
                                if !text.trim().is_empty() {
                                    authors_list.push(text.trim().to_string());
                                }
                            }
                            _ => {}
                        }
                    } else if (current_tag == "lastBuildDate"
                        || current_tag == "dc:date"
                        || current_tag == "pubDate")
                        && channel_update_time.is_none()
                    {
                        channel_update_time = Some(text);
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "item" || name == "entry" {
                        in_item = false;

                        // 1. 确定 ID (DOI > GUID > Link)
                        let mut doi = String::new();

                        // 尝试从 prism:doi 获取
                        if !prism_doi.is_empty() {
                            doi = prism_doi.trim_start_matches("doi:").to_string();
                        }
                        // 尝试从 dc:identifier 获取
                        else if !dc_identifier.is_empty() {
                            if dc_identifier.to_lowercase().starts_with("doi:") {
                                doi = dc_identifier[4..].trim().to_string();
                            } else if dc_identifier.contains("10.") {
                                doi = dc_identifier.trim().to_string();
                            }
                        }
                        // 尝试从 GUID 获取 (如果是 DOI 格式)
                        else if guid.contains("10.") && !guid.starts_with("http") {
                            if guid.to_lowercase().starts_with("doi:") {
                                doi = guid[4..].trim().to_string();
                            } else {
                                doi = guid.trim().to_string();
                            }
                        }

                        let item_id = if !doi.is_empty() {
                            doi.clone()
                        } else if !guid.is_empty() && !guid.contains("10.") {
                            guid.clone()
                        } else if !link.is_empty() {
                            link.clone()
                        } else {
                            Uuid::new_v4().to_string()
                        };

                        let mut item = create_feed_item(
                            item_id,
                            text::clean_title(&title),
                            feed_id.to_string(),
                        );

                        // 2. 填充基本信息
                        item.url = text::clean_optional_text(Some(&link));
                        if !doi.is_empty() {
                            item.doi = Some(doi);
                        }

                        // 摘要优先使用 content:encoded (HTML)，其次 description
                        // 许多学术 RSS (如 ACS, Springer) 在 description 中包含 HTML 标签（图片、格式），需要清理
                        let raw_abstract = if content_encoded.is_empty() {
                            &description
                        } else {
                            &content_encoded
                        };

                        // 先移除 HTML 标签，再清理空白字符
                        let cleaned_abstract = text::strip_html_tags(raw_abstract);
                        item.abstract_text = if cleaned_abstract.is_empty() {
                            None
                        } else {
                            Some(cleaned_abstract)
                        };

                        // 出版信息
                        item.journal = text::clean_optional_text(Some(&journal));
                        item.volume = text::clean_optional_text(Some(&volume));
                        item.issue = text::clean_optional_text(Some(&issue));

                        // 3. 确定日期 (PRISM > DC > RSS)
                        let date_str = if !prism_date.is_empty() {
                            prism_date.clone()
                        } else if !dc_date.is_empty() {
                            dc_date.clone()
                        } else {
                            pub_date.clone()
                        };

                        if !date_str.is_empty() {
                            item.published_at = Some(crate::time::normalize_time_string(&date_str));
                        }

                        // 4. 处理作者
                        // 策略：
                        // - MDPI/RSC 使用分号 (;) 分隔
                        // - 大多数其他出版商使用逗号 (,) 分隔
                        // - T&F 混杂机构且无逗号，尝试启发式清理

                        for auth_str in &authors_list {
                            // 清理可能的机构后缀 (简单的启发式)
                            let clean_auth = if let Some(idx) = auth_str.find(" a School") {
                                &auth_str[..idx]
                            } else if let Some(idx) = auth_str.find(" Department") {
                                &auth_str[..idx]
                            } else {
                                auth_str
                            };

                            // MDPI, RSC 等使用分号分隔
                            if clean_auth.contains(';') {
                                for name in clean_auth.split(';') {
                                    process_author_name(name, &mut item.authors);
                                }
                            }
                            // 大多数其他来源使用逗号分隔
                            else if clean_auth.contains(',') {
                                for name in clean_auth.split(',') {
                                    process_author_name(name, &mut item.authors);
                                }
                            }
                            // 单个名字或无法识别分隔符
                            else {
                                process_author_name(clean_auth, &mut item.authors);
                            }
                        }

                        debug!(
                            "通用解析器: 解析到条目: {} (DOI: {:?})",
                            item.title, item.doi
                        );
                        items.push(item);
                    }
                    current_tag.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    error!("RSS XML 解析错误: {e}");
                    return Err(anyhow!("XML 解析错误: {e}"));
                }
                _ => {}
            }
            buf.clear();
        }

        info!("通用 RSS 解析完成, 共 {} 条", items.len());
        Ok((items, channel_update_time))
    }
}

/// 处理单个作者姓名并添加到列表
fn process_author_name(raw_name: &str, authors: &mut Vec<Author>) {
    let name = raw_name.trim();
    if name.is_empty() {
        return;
    }

    let parts: Vec<&str> = name.split_whitespace().collect();
    if !parts.is_empty() {
        let last = text::clean_author_name(parts.last().unwrap_or(&""));
        let first = text::clean_author_name(&parts[..parts.len() - 1].join(" "));
        authors.push(create_author(last, first));
    }
}
