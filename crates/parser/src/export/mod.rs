//! 导出模块
//!
//! 负责将文献数据导出为多种标准引用格式

pub mod bibtex;
pub mod elsevier;
pub mod ieee;

pub use bibtex::BibTeXExporter;
pub use elsevier::ElsevierExporter;
pub use ieee::IeeeExporter;

use anyhow::Result;
use log::info;
use models::Literature;
use std::fs;
use std::path::Path;

/// 导出器基础 Trait
pub trait Exporter: Send + Sync {
    fn format_name(&self) -> &str;
    fn export_to_string(&self, items: &[Literature]) -> Result<String>;
}

/// 导出格式枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    IEEE,
    BibTeX,
    Elsevier,
}

impl ExportFormat {
    #[must_use]
    pub fn extension(&self) -> &str {
        match self {
            ExportFormat::IEEE | ExportFormat::Elsevier => "txt",
            ExportFormat::BibTeX => "bib",
        }
    }
}

/// 导出管理器
pub struct ExportManager;

impl ExportManager {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 执行导出操作
    pub fn export(
        &self,
        format: ExportFormat,
        items: &[Literature],
        output_path: &Path,
    ) -> Result<()> {
        let content = self.export_to_string(format, items)?;
        info!(
            "导出: {}格式 -> {:?} ({} 篇文献)",
            self.format_name(format),
            output_path,
            items.len()
        );
        fs::write(output_path, content)?;
        Ok(())
    }

    /// 转换为格式化字符串
    fn format_name(&self, format: ExportFormat) -> &str {
        match format {
            ExportFormat::IEEE => "IEEE",
            ExportFormat::BibTeX => "BibTeX",
            ExportFormat::Elsevier => "Elsevier",
        }
    }

    pub fn export_to_string(&self, format: ExportFormat, items: &[Literature]) -> Result<String> {
        let exporter: Box<dyn Exporter> = match format {
            ExportFormat::IEEE => Box::new(IeeeExporter),
            ExportFormat::BibTeX => Box::new(BibTeXExporter),
            ExportFormat::Elsevier => Box::new(ElsevierExporter),
        };
        exporter.export_to_string(items)
    }

    #[must_use]
    pub fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![
            ExportFormat::IEEE,
            ExportFormat::BibTeX,
            ExportFormat::Elsevier,
        ]
    }
}

impl Default for ExportManager {
    fn default() -> Self {
        Self::new()
    }
}
