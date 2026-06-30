use crate::text;
use crate::{MetadataParser, normalize};
use anyhow::{Result, anyhow};
use biblatex::{Bibliography, ChunksExt, DateValue, Entry, EntryType, PermissiveType};
use database::constructors::*;
use log::{debug, error, info, warn};
use models::{Literature, LiteratureType, PublicationType};
use uuid::Uuid;

pub struct BibTeXParser;

impl MetadataParser for BibTeXParser {
    fn source_id(&self) -> &'static str {
        "BibTeX"
    }

    async fn parse(&self, input: &str) -> Result<Vec<Literature>> {
        Self::parse(input)
    }
}

impl BibTeXParser {
    pub fn parse(content: &str) -> Result<Vec<Literature>> {
        info!(
            "解析器: [BibTeX] 正在解析内容, 长度: {} 字符",
            content.len()
        );
        let bib = Bibliography::parse(content).map_err(|e| {
            error!("解析器: [BibTeX] 语法解析失败: {e:?}");
            anyhow!("Failed to parse BibTeX: {e:?}")
        })?;

        let mut results = Vec::new();
        for entry in bib.iter() {
            match Self::convert_entry(entry) {
                Ok(lit) => {
                    debug!("解析器: [BibTeX] 成功解析出文献: '{}'", lit.title);
                    results.push(lit);
                }
                Err(e) => {
                    warn!("解析器: [BibTeX] 跳过解析失败的条目: {e}");
                }
            }
        }

        info!(
            "解析器: [BibTeX] 解析完成，共成功解析出 {} 条文献",
            results.len()
        );
        Ok(results)
    }

    fn convert_entry(entry: &Entry) -> Result<Literature> {
        let title_raw = entry.title().map_or_else(
            |_| "Untitled".to_string(),
            biblatex::ChunksExt::format_verbatim,
        );

        let title = text::clean_title(&title_raw);

        let lit_type = match entry.entry_type {
            EntryType::Article => LiteratureType::Article,
            EntryType::Book => LiteratureType::Book,
            EntryType::InProceedings | EntryType::Proceedings => LiteratureType::Conference,
            EntryType::Thesis | EntryType::MastersThesis | EntryType::PhdThesis => {
                LiteratureType::Thesis
            }
            EntryType::Misc => LiteratureType::Other,
            _ => LiteratureType::Article,
        };

        let mut lit = create_literature(Uuid::new_v4().to_string(), title, lit_type);

        // Authors
        if let Ok(authors) = entry.author() {
            for person in authors {
                lit.authors.push(create_author(
                    text::clean_author_name(&person.name),
                    text::clean_author_name(&person.given_name),
                ));
            }
            debug!("解析器: [BibTeX] 解析出 {} 位作者", lit.authors.len());
        }

        // Year
        if let Ok(permissive_date) = entry.date() {
            match permissive_date {
                PermissiveType::Typed(date) => {
                    let year = match date.value {
                        DateValue::At(dt) => Some(dt.year),
                        DateValue::After(dt) => Some(dt.year),
                        DateValue::Before(dt) => Some(dt.year),
                        DateValue::Between(dt, _) => Some(dt.year),
                    };
                    lit.year = year;
                }
                PermissiveType::Chunks(chunks) => {
                    if let Ok(year_val) = chunks.format_verbatim().trim().parse::<i32>() {
                        lit.year = Some(year_val);
                    }
                }
            }
            if let Some(y) = lit.year {
                debug!("解析器: [BibTeX] 解析年份: {y}");
            }
        }

        // Helper to get field as string
        let get_field = |key: &str| -> Option<String> {
            entry.get(key).map(biblatex::ChunksExt::format_verbatim)
        };

        // 使用 Publication 字段
        if let Some(journal) = get_field("journal")
            .map(|s| text::clean_publication_name(&s))
            .filter(|s| !s.is_empty())
        {
            lit.publication = Some(create_publication(journal, PublicationType::Journal));
        } else if let Some(booktitle) = get_field("booktitle")
            .map(|s| text::clean_title(&s))
            .filter(|s| !s.is_empty())
        {
            // booktitle 根据文献类型选择 Conference 或 Book
            let pub_type = if lit.literature_type == models::LiteratureType::Conference {
                PublicationType::Conference
            } else {
                PublicationType::Book
            };
            lit.publication = Some(create_publication(booktitle, pub_type));
        }
        lit.volume = text::clean_optional_text(get_field("volume").as_deref());
        // BibTeX 标准字段名为 "number"，部分来源也使用 "issue"，两者均尝试
        lit.issue = text::clean_optional_text(
            get_field("number")
                .as_deref()
                .or(get_field("issue").as_deref()),
        );
        lit.pages = text::clean_optional_page_range(get_field("pages").as_deref());

        // Set publisher on publication object
        if let Some(publisher_str) = text::clean_optional_text(get_field("publisher").as_deref()) {
            if let Some(ref mut pub_obj) = lit.publication {
                pub_obj.publisher = Some(publisher_str);
            } else {
                // Create a new Publication if none exists
                let pub_type = if lit.literature_type == models::LiteratureType::Conference {
                    PublicationType::Conference
                } else {
                    PublicationType::Journal
                };
                let mut new_pub = create_publication(String::new(), pub_type);
                new_pub.publisher = Some(publisher_str);
                lit.publication = Some(new_pub);
            }
        }

        lit.doi = text::clean_optional_text(get_field("doi").as_deref());
        lit.isbn = text::clean_optional_text(get_field("isbn").as_deref());
        lit.url = text::clean_optional_text(get_field("url").as_deref());
        lit.abstract_text = text::clean_optional_text(get_field("abstract").as_deref());

        // 自动规范化标识符
        normalize::sanitize_arxiv_identifiers(&mut lit);

        // Handle eprint field (common in BibTeX for arXiv)
        if lit.arxiv_id.is_none()
            && let Some(eprint) = get_field("eprint")
        {
            lit.arxiv_id = Some(eprint.trim().replace("arXiv:", ""));
        }

        Ok(lit)
    }
}
