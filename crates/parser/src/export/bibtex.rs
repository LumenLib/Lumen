use super::{Exporter, publication_display_name};
use anyhow::Result;
use models::{Literature, LiteratureType, PublicationType};
use regex::Regex;
use std::fmt::Write;
use std::sync::LazyLock;

/// 匹配连字符串（长度>=1）。仅将长度为 1 的连字符转成 LaTeX 页码区间的 `--`，
/// 已存在的 `--` 等原样保留。用运行长度判断替代环视断言（regex crate 不支持）。
static RE_HYPHEN_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-+").unwrap());

/// BibTeX 格式导出器
pub struct BibTeXExporter;

impl BibTeXExporter {
    fn lit_to_bib(&self, lit: &Literature, abbreviate_journal: bool) -> String {
        let entry_type = match lit.literature_type {
            LiteratureType::Article => "article",
            LiteratureType::Conference => "inproceedings",
            LiteratureType::Book => "book",
            LiteratureType::Thesis => "phdthesis",
            _ => "misc",
        };

        let first_author = lit
            .authors
            .first()
            .map_or_else(|| "unknown".into(), |a| a.last_name.to_lowercase());
        let year = lit.year.unwrap_or(0);
        let key = format!("{first_author}{year}");

        let mut s = format!("@{entry_type}{{{key},\n");
        writeln!(s, "  title = {{{}}},", lit.title).unwrap();

        let authors = lit
            .authors
            .iter()
            .map(|a| format!("{}, {}", a.last_name, a.first_name))
            .collect::<Vec<_>>()
            .join(" and ");
        writeln!(s, "  author = {{{authors}}},").unwrap();

        // 使用 publication 字段代替旧的 journal/conference 字段
        if let Some(ref pub_info) = lit.publication {
            let name = publication_display_name(pub_info, abbreviate_journal);
            match pub_info.publication_type {
                PublicationType::Journal => {
                    writeln!(s, "  journal = {{{name}}},").unwrap();
                }
                PublicationType::Conference => {
                    writeln!(s, "  booktitle = {{{name}}},").unwrap();
                }
                PublicationType::Book => {
                    writeln!(s, "  booktitle = {{{name}}},").unwrap();
                }
            }
        }
        if let Some(year) = lit.year {
            writeln!(s, "  year = {{{year}}},").unwrap();
        }
        if let Some(ref v) = lit.volume {
            writeln!(s, "  volume = {{{v}}},").unwrap();
        }
        if let Some(ref n) = lit.issue {
            writeln!(s, "  number = {{{n}}},").unwrap();
        }
        if let Some(ref p) = lit.pages {
            let pages = RE_HYPHEN_RUN.replace_all(p, |caps: &regex::Captures| {
                if caps[0].len() == 1 {
                    "--".to_string()
                } else {
                    caps[0].to_string()
                }
            });
            writeln!(s, "  pages = {{{pages}}},").unwrap();
        }
        if let Some(ref d) = lit.doi {
            writeln!(s, "  doi = {{{d}}},").unwrap();
        }

        s.push('}');
        s
    }
}

impl Exporter for BibTeXExporter {
    fn format_name(&self) -> &'static str {
        "BibTeX"
    }
    fn export_to_string(&self, items: &[Literature], abbreviate_journal: bool) -> Result<String> {
        let entries: Vec<String> = items
            .iter()
            .map(|lit| self.lit_to_bib(lit, abbreviate_journal))
            .collect();
        Ok(entries.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize_pages(pages: &str) -> String {
        RE_HYPHEN_RUN
            .replace_all(pages, |caps: &regex::Captures| {
                if caps[0].len() == 1 {
                    "--".to_string()
                } else {
                    caps[0].to_string()
                }
            })
            .into_owned()
    }

    #[test]
    fn test_single_hyphen_becomes_double() {
        assert_eq!(normalize_pages("3-4"), "3--4");
        assert_eq!(normalize_pages("1024-1030"), "1024--1030");
    }

    #[test]
    fn test_existing_double_hyphen_preserved() {
        assert_eq!(normalize_pages("3--4"), "3--4");
        assert_eq!(normalize_pages("3--4-5"), "3--4--5");
    }
}
