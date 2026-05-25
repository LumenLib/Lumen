use super::Exporter;
use anyhow::Result;
use models::Literature;
use std::fmt::Write;

/// Elsevier (Numbering) 格式导出器
/// 格式示例: [1] J. Doe, J. Smith, Title of Paper, Journal Name 1 (2) (2024) 3-4.
pub struct ElsevierExporter;

impl ElsevierExporter {
    fn format_elsevier(&self, index: usize, lit: &Literature) -> String {
        let mut s = String::new();
        write!(s, "[{}] ", index + 1).unwrap();

        // Authors: J. Doe, J. Smith,
        let authors = lit
            .authors
            .iter()
            .map(|a| {
                let first_initial = a
                    .first_name
                    .chars()
                    .next()
                    .map(|c| format!("{c}."))
                    .unwrap_or_default();
                if first_initial.is_empty() {
                    a.last_name.clone()
                } else {
                    format!("{} {}", first_initial, a.last_name)
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        if !authors.is_empty() {
            s.push_str(&authors);
            s.push_str(", ");
        }

        // Title
        s.push_str(&lit.title);
        s.push_str(", ");

        // Journal and Date info: Journal Name Vol (Issue) (Year) Pages
        let venue = lit
            .publication
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if !venue.is_empty() {
            s.push_str(&venue);
            s.push(' ');
        }

        if let Some(ref v) = lit.volume {
            s.push_str(v);
            s.push(' ');
        }

        if let Some(ref n) = lit.issue {
            write!(s, "({n}) ").unwrap();
        }

        if let Some(year) = lit.year {
            write!(s, "({year}) ").unwrap();
        }

        if let Some(ref p) = lit.pages {
            s.push_str(p);
        }

        // 移除末尾多余的空格
        while s.ends_with(' ') {
            s.pop();
        }

        s.push('.');
        s
    }
}

impl Exporter for ElsevierExporter {
    fn format_name(&self) -> &'static str {
        "Elsevier"
    }
    fn export_to_string(&self, items: &[Literature]) -> Result<String> {
        let lines: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, lit)| self.format_elsevier(i, lit))
            .collect();
        Ok(lines.join("\n"))
    }
}
