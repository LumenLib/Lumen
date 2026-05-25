use super::Exporter;
use anyhow::Result;
use models::Literature;
use std::fmt::Write;

/// IEEE Transactions 引用格式导出器
/// 格式示例: [1] J. Doe and J. Smith, "Title of Paper," Journal Name, vol. 1, no. 2, pp. 3-4, 2024.
pub struct IeeeExporter;

impl IeeeExporter {
    fn format_ieee(&self, index: usize, lit: &Literature) -> String {
        let mut s = String::new();
        write!(s, "[{}] ", index + 1).unwrap();

        // Authors: J. Smith, A. Taylor, and ...
        let authors = lit
            .authors
            .iter()
            .map(|a| {
                let first_initial = a
                    .first_name
                    .chars()
                    .next()
                    .map(|c| format!("{c}. "))
                    .unwrap_or_default();
                format!("{}{}", first_initial, a.last_name)
            })
            .collect::<Vec<_>>();

        if !authors.is_empty() {
            if authors.len() == 1 {
                s.push_str(&authors[0]);
            } else if authors.len() == 2 {
                s.push_str(&format!("{} and {}", authors[0], authors[1]));
            } else {
                for (i, name) in authors.iter().enumerate() {
                    if i == authors.len() - 1 {
                        s.push_str(&format!(", and {name}"));
                    } else {
                        if i > 0 {
                            s.push_str(", ");
                        }
                        s.push_str(name);
                    }
                }
            }
            s.push_str(", ");
        }

        // Title
        write!(s, "\"{},\" ", lit.title).unwrap();

        // Venue (使用 publication 字段)
        let venue = lit
            .publication
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if !venue.is_empty() {
            write!(s, "{venue}, ").unwrap();
        }

        // Volume, Issue, Pages
        if let Some(ref vol) = lit.volume {
            write!(s, "vol. {vol}, ").unwrap();
        }
        if let Some(ref issue) = lit.issue {
            write!(s, "no. {issue}, ").unwrap();
        }
        if let Some(ref pages) = lit.pages {
            write!(s, "pp. {pages}, ").unwrap();
        }

        // Year
        if let Some(year) = lit.year {
            write!(s, "{year}.").unwrap();
        } else {
            s.push('.');
        }

        s
    }
}

impl Exporter for IeeeExporter {
    fn format_name(&self) -> &'static str {
        "IEEE Transactions"
    }
    fn export_to_string(&self, items: &[Literature]) -> Result<String> {
        let lines: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, lit)| self.format_ieee(i, lit))
            .collect();
        Ok(lines.join("\n"))
    }
}
