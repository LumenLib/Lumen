use anyhow::{Context, Result};
use log::{debug, error, info};
use models::constructors::*;
use models::{Author, FeedItem};
use quick_xml::de::from_str;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Rss {
    channel: Channel,
}

#[derive(Debug, Deserialize)]
struct Channel {
    title: String,
    #[serde(rename = "pubDate", default)]
    pub_date: Option<String>,
    #[serde(rename = "item", default)]
    items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    title: String,
    link: String,
    description: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    #[serde(rename = "pubYear")]
    pub_year: Option<String>,
    authors: Option<String>,
    #[serde(default)]
    doi: Option<String>,
    #[serde(default)]
    volume: Option<String>,
    #[serde(default)]
    issue: Option<String>,
    #[serde(rename = "startPage", default)]
    start_page: Option<String>,
    #[serde(rename = "endPage", default)]
    end_page: Option<String>,
}

/// IEEE 订阅解析器
///
/// 负责从 IEEE 相关的列表源（如搜索结果、RSS、专题页）提取初步的 `FeedItem` 信息
pub struct IeeeSubscriptionParser;

impl IeeeSubscriptionParser {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for IeeeSubscriptionParser {
    fn default() -> Self {
        Self::new()
    }
}

impl IeeeSubscriptionParser {
    /// 解析列表内容，返回 `(条目列表, 频道标题, 源更新时间)`
    pub fn parse_list(
        &self,
        content: &str,
        feed_id: &str,
    ) -> Result<(Vec<FeedItem>, Option<String>, Option<String>)> {
        info!("开始解析 IEEE 订阅源: {feed_id}");
        let rss: Rss = from_str(content).with_context(|| {
            error!("IEEE XML 反序列化失败: {feed_id}");
            format!("IEEE XML 反序列化失败: {feed_id}")
        })?;
        let journal_name = rss
            .channel
            .title
            .replace(" - new TOC", "")
            .trim()
            .to_string();
        let pub_date = rss
            .channel
            .pub_date
            .map(|s| crate::time::normalize_time_string(&s));

        let mut feed_items = Vec::new();

        for item in rss.channel.items {
            // ... (previous logic for filtering) ...
            let title = item.title.trim();
            if title == "Front Cover"
                || title == "Table of Contents"
                || title.contains("Information for Authors")
                || title.contains("Publication Information")
                || title.contains("Society Information")
            {
                continue;
            }

            // 使用 URL 或 DOI 作为基础 ID
            let mut item_id = item.doi.clone().unwrap_or_else(|| item.link.clone());
            if item_id.is_empty() {
                item_id = Uuid::new_v4().to_string();
            }

            let mut feed_item = create_feed_item(item_id, title, feed_id);

            feed_item.url = Some(item.link.clone());
            feed_item.journal = Some(journal_name.clone());
            feed_item.abstract_text = item.description;
            feed_item.volume = item.volume;
            feed_item.issue = item.issue;
            feed_item.published_at = item
                .pub_date
                .map(|s| crate::time::normalize_time_string(&s));

            // 合并页码
            if let Some(sp) = item.start_page {
                if let Some(ep) = item.end_page {
                    feed_item.pages = Some(format!("{sp}-{ep}"));
                } else {
                    feed_item.pages = Some(sp);
                }
            }

            feed_item.doi = item.doi;

            // 解析年份
            if let Some(year_str) = item.pub_year
                && let Ok(year) = year_str.trim().parse::<i32>()
            {
                feed_item.year = Some(year);
            }

            // 解析作者 (格式: Author1;Author2;)
            if let Some(authors_str) = item.authors {
                let authors: Vec<Author> = authors_str
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|name| {
                        let parts: Vec<&str> = name.split_whitespace().collect();
                        if parts.len() > 1 {
                            let last_name = parts.last().unwrap_or(&"").to_string();
                            let first_name = parts[..parts.len() - 1].join(" ");
                            create_author(last_name, first_name)
                        } else {
                            create_author(name.to_string(), "")
                        }
                    })
                    .collect();
                feed_item.authors = authors;
            }

            debug!(
                "解析到 IEEE 条目: {} (DOI: {:?})",
                feed_item.title, feed_item.doi
            );
            feed_items.push(feed_item);
        }

        info!(
            "IEEE 解析完成, 共获取 {} 条文献, 频道标题: {:?}, 更新时间: {:?}",
            feed_items.len(),
            journal_name,
            pub_date
        );
        Ok((feed_items, Some(journal_name), pub_date))
    }
}
