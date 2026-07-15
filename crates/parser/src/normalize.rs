use log::debug;
use models::{Author, Literature};
use regex::Regex;

pub fn sanitize_arxiv_identifiers(lit: &mut Literature) {
    let title_preview: String = lit.title.chars().take(60).collect();
    debug!("ID 规范化: 处理文献 '{}' (ID: {})", title_preview, lit.id);
    if lit.doi.is_none() {
        if let Some(url) = &lit.url
            && (url.contains("doi.org/") || url.contains("dx.doi.org/"))
        {
            let re = Regex::new(r"(?:dx\.)?doi\.org/(.+)").unwrap();
            if let Some(caps) = re.captures(url) {
                let doi = caps.get(1).unwrap().as_str().trim();
                let doi = doi.split('?').next().unwrap_or(doi);
                if !doi.is_empty() {
                    lit.doi = Some(doi.to_string());
                    lit.url = None;
                }
            }
        }
    } else if let Some(url) = &lit.url
        && (url.contains("doi.org/") || url.contains("dx.doi.org/"))
    {
        lit.url = None;
    }

    if let Some(doi) = &lit.doi
        && doi.starts_with("10.48550/arXiv.")
    {
        let id = doi.replace("10.48550/arXiv.", "").trim().to_string();
        if lit.arxiv_id.is_none() {
            lit.arxiv_id = Some(id);
        }
        lit.doi = None;
    }

    if let Some(url) = &lit.url
        && url.contains("arxiv.org")
    {
        if lit.arxiv_id.is_none() {
            let re = Regex::new(r"(\d{4}\.\d{4,5})").unwrap();
            if let Some(caps) = re.captures(url) {
                lit.arxiv_id = Some(caps.get(1).unwrap().as_str().to_string());
            } else {
                let old_re = Regex::new(r"([a-z\-]+/\d{7})").unwrap();
                if let Some(caps) = old_re.captures(url) {
                    lit.arxiv_id = Some(caps.get(1).unwrap().as_str().to_string());
                }
            }
        }
        lit.url = None;
    }

    if let Some(doi) = &lit.doi {
        let mut cleaned = doi.trim();
        if cleaned.to_lowercase().starts_with("doi:") {
            cleaned = &cleaned[4..];
        }
        let cleaned = cleaned.trim().to_string();
        lit.doi = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        };
    }

    if let Some(id) = &lit.arxiv_id {
        let mut cleaned = id.trim();
        if cleaned.to_lowercase().starts_with("arxiv:") {
            cleaned = &cleaned[6..];
        }
        let cleaned = cleaned.trim().to_string();
        lit.arxiv_id = if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        };
    }

    // 去除标题末尾多余的句点
    let trimmed_title = lit.title.trim_end_matches('.').trim().to_string();
    if trimmed_title != lit.title {
        debug!("标题规范化: 去除末尾句点 '{}'", lit.title);
        lit.title = trimmed_title;
    }
}

pub fn author_full_name(author: &Author) -> String {
    if let Some(ref middle) = author.middle_name {
        format!("{} {} {}", author.first_name, middle, author.last_name)
    } else {
        format!("{} {}", author.first_name, author.last_name)
    }
}

pub fn parse_author_list(s: &str) -> Vec<Author> {
    debug!("作者解析: 输入=\"{}\"", s);
    let result: Vec<Author> = s
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| {
            let parts: Vec<&str> = name.split_whitespace().collect();
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            if parts.len() >= 2 {
                Author {
                    id: uuid::Uuid::new_v4().to_string(),
                    last_name: parts.last().unwrap().to_string(),
                    first_name: parts[0].to_string(),
                    middle_name: None,
                    is_dirty: true,
                    is_deleted: false,
                    version: 1,
                    created_at: now.clone(),
                    updated_at: now,
                }
            } else {
                Author {
                    id: uuid::Uuid::new_v4().to_string(),
                    last_name: name.to_string(),
                    first_name: String::new(),
                    middle_name: None,
                    is_dirty: true,
                    is_deleted: false,
                    version: 1,
                    created_at: now.clone(),
                    updated_at: now,
                }
            }
        })
        .collect();
    debug!("作者解析: 解析出 {} 位作者", result.len());
    result
}
