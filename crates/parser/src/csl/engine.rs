//! CSL 渲染引擎模块
//!
//! 封装 hayagriva 引擎，提供引文格式化功能

use crate::csl::converter::literature_to_entry;
use crate::csl::registry::REGISTRY;
use anyhow::{Context, Result};
use hayagriva::{
    BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest,
    citationberg::{IndependentStyle, Locale},
};
use log::{debug, error};
use models::Literature;

#[derive(Clone, PartialEq)]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
}

/// 获取所有可用样式
pub fn available_styles() -> Vec<StyleInfo> {
    match REGISTRY.read() {
        Ok(registry) => registry
            .list_styles()
            .into_iter()
            .map(|(id, name)| StyleInfo { id, name })
            .collect(),
        Err(e) => {
            error!("CSL registry lock is poisoned: {e}");
            vec![]
        }
    }
}

/// 格式化单条引文
pub fn format_citation(lit: &Literature, style_id: &str, _locale: &str) -> Result<String> {
    format_bibliography(std::slice::from_ref(lit), style_id)
}

/// 格式化多条文献为参考文献列表
pub fn format_bibliography(lits: &[Literature], style_id: &str) -> Result<String> {
    debug!("CSL 格式化: {} 篇文献, 样式={}", lits.len(), style_id);
    if lits.is_empty() {
        return Ok(String::new());
    }

    let style_xml = get_style_xml(style_id).context("Style not found")?;
    let style = IndependentStyle::from_xml(&style_xml)
        .map_err(|e| anyhow::anyhow!("Failed to parse CSL style: {e}"))?;

    // 从注册表获取区域设置
    let locales = match REGISTRY.write() {
        Ok(mut registry) => {
            let locale_file = registry.get_default_locale();
            let locale: Locale = locale_file.into();
            vec![locale]
        }
        Err(_) => vec![],
    };

    let mut driver = BibliographyDriver::new();

    // 提前转换所有 Entry
    let entries: Vec<_> = lits.iter().map(literature_to_entry).collect();

    // 生成引文项
    for entry in &entries {
        let items = vec![CitationItem::with_entry(entry)];
        let citation_request = CitationRequest::from_items(items, &style, &locales);
        driver.citation(citation_request);
    }

    let bib_request = BibliographyRequest {
        style: &style,
        locale: None,
        locale_files: &locales,
    };

    let output = driver.finish(bib_request);

    // 提取并合并所有参考文献条目
    if let Some(bib) = output.bibliography {
        let mut result = String::new();
        for item in bib.items {
            let first = item
                .first_field
                .as_ref()
                .map(|f| strip_formatting(&format!("{f} ")))
                .unwrap_or_default();
            let content = strip_formatting(&format!("{}", item.content));
            result.push_str(&format!("{first}{content}\n"));
        }
        Ok(result.trim().to_string())
    } else {
        // 退而求其次使用引文预览
        let mut result = String::new();
        for citation in output.citations {
            result.push_str(&format!(
                "{}\n",
                strip_formatting(&format!("{}", citation.citation))
            ));
        }
        Ok(result.trim().to_string())
    }
}

/// 移除 ANSI 转义代码（hayagriva 默认输出包含 ANSI 样式）
fn strip_formatting(text: &str) -> String {
    let ansi_regex = regex::Regex::new(
        r"[\u001b\u009b](\[[0-?]*[ -/]*[@-~]|\].*?([\u001b\u009b]\\|\x07)|[@-Z\\-_])",
    )
    .unwrap();
    ansi_regex.replace_all(text, "").to_string()
}

/// 获取样式 XML
fn get_style_xml(style_id: &str) -> Option<String> {
    match REGISTRY.read() {
        Ok(registry) => registry.get_resolved_style_xml(style_id),
        Err(e) => {
            error!("CSL registry lock is poisoned: {e}");
            None
        }
    }
}
