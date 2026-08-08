use super::Exporter;
use anyhow::Result;
use models::{Literature, LiteratureType, PublicationType};
use regex::Regex;
use std::fmt::Write;
use std::sync::LazyLock;

/// 匹配单独出现的连字符（排除已存在的 `--`），用于转成 LaTeX 页码区间的 `--`
static RE_SINGLE_HYPHEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?<!-)-(?!-)").unwrap());

/// BibTeX 格式导出器
pub struct BibTeXExporter;

impl BibTeXExporter {
    fn lit_to_bib(&self, lit: &Literature) -> String {
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
            match pub_info.publication_type {
                PublicationType::Journal => {
                    writeln!(s, "  journal = {{{}}},", pub_info.name).unwrap();
                }
                PublicationType::Conference => {
                    writeln!(s, "  booktitle = {{{}}},", pub_info.name).unwrap();
                }
                PublicationType::Book => {
                    writeln!(s, "  booktitle = {{{}}},", pub_info.name).unwrap();
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
            let pages = RE_SINGLE_HYPHEN.replace_all(p, "--");
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
    fn export_to_string(&self, items: &[Literature]) -> Result<String> {
        let entries: Vec<String> = items.iter().map(|lit| self.lit_to_bib(lit)).collect();
        Ok(entries.join("\n\n"))
    }
}
