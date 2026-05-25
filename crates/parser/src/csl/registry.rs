//! 样式管理模块

use hayagriva::citationberg::{InfoLinkRel, LocaleFile, Style};
use log::{error, info};
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{LazyLock, RwLock},
};

pub struct StyleRegistry {
    /// 原始 XML 存储: filename -> xml
    styles: HashMap<String, String>,
    /// URL ID 到 XML 的映射: <http://.../ieee> -> xml
    id_map: HashMap<String, String>,
    /// 默认区域设置缓存
    default_locale: Option<LocaleFile>,
}

impl Default for StyleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl StyleRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
            id_map: HashMap::new(),
            default_locale: None,
        }
    }

    /// 获取默认区域设置 (en-US)
    pub fn get_default_locale(&mut self) -> LocaleFile {
        if let Some(locale) = &self.default_locale {
            return locale.clone();
        }

        let en_locale = r#"<?xml version="1.0" encoding="utf-8"?>
<locale xmlns="http://www.citationstyles.org/styles/1.0/" xml:lang="en-US" version="1.0">
  <info><translator><name>System</name></translator></info>
  <style-options punctuation-in-quote="true"/>
  <date form="text">
    <date-part name="month" suffix=" "/>
    <date-part name="day" suffix=", "/>
    <date-part name="year"/>
  </date>
  <terms>
    <term name="open-quote">“</term>
    <term name="close-quote">”</term>
    <term name="open-inner-quote">‘</term>
    <term name="close-inner-quote">’</term>
    <term name="and">and</term>
    <term name="et-al">et al.</term>
    <term name="volume" form="short">vol.</term>
    <term name="issue" form="short">no.</term>
    <term name="page" form="short">
      <single>p.</single>
      <multiple>pp.</multiple>
    </term>
  </terms>
</locale>"#;

        let locale = LocaleFile::from_xml(en_locale).expect("Failed to parse builtin locale");
        self.default_locale = Some(locale.clone());
        locale
    }

    /// 从目录加载 CSL 文件
    pub fn load_from_dir(&mut self, dir: &Path) {
        if !dir.exists() {
            info!("CSL directory does not exist: {dir:?}");
            return;
        }

        info!("Loading CSL styles from: {dir:?}");
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "csl")
                    && let Some(name) = path.file_stem().and_then(|s| s.to_str())
                {
                    match fs::read_to_string(&path) {
                        Ok(xml) => {
                            // 解析 ID
                            if let Ok(style) = Style::from_xml(&xml) {
                                let id = match &style {
                                    Style::Independent(s) => s.info.id.clone(),
                                    Style::Dependent(s) => s.info.id.clone(),
                                };
                                self.id_map.insert(id, xml.clone());
                            }
                            self.styles.insert(name.to_string(), xml);
                        }
                        Err(e) => {
                            error!("Failed to read CSL file {path:?}: {e}");
                        }
                    }
                }
            }
        }
    }

    /// 获取最终可用的独立样式 XML
    /// 如果是 dependent 样式，则递归查找其 parent
    #[must_use]
    pub fn get_resolved_style_xml(&self, name: &str) -> Option<String> {
        let xml = self.styles.get(name)?;
        self.resolve_xml(xml)
    }

    fn resolve_xml(&self, xml: &str) -> Option<String> {
        match Style::from_xml(xml) {
            Ok(Style::Independent(_)) => Some(xml.to_string()),
            Ok(Style::Dependent(dep)) => {
                // 查找父级 ID
                let parent_id = dep
                    .info
                    .link
                    .iter()
                    .find(|l| matches!(l.rel, InfoLinkRel::IndependentParent))
                    .map(|l| l.href.as_str())?;

                info!("Resolving dependent style, parent: {parent_id}");

                // 从 id_map 中查找父级 XML
                let parent_xml = self.id_map.get(parent_id)?;
                // 递归解析（防止多级依赖）
                self.resolve_xml(parent_xml)
            }
            Err(e) => {
                error!("Failed to parse style XML: {e}");
                None
            }
        }
    }

    /// 获取所有样式的列表 (id, title)
    #[must_use]
    pub fn list_styles(&self) -> Vec<(String, String)> {
        let mut list: Vec<_> = self
            .styles
            .iter()
            .map(|(k, v)| {
                let title = match Style::from_xml(v) {
                    Ok(Style::Independent(s)) => s.info.title.value.clone(),
                    Ok(Style::Dependent(s)) => s.info.title.value.clone(),
                    Err(_) => k.clone(),
                };
                (k.clone(), title)
            })
            .collect();

        list.sort_by(|a, b| a.1.cmp(&b.1));
        list
    }

    /// 从文件热加载 CSL 样式
    pub fn reload_style_from_file(&mut self, path: &Path) {
        if path.extension().is_some_and(|ext| ext == "csl")
            && let Some(name) = path.file_stem().and_then(|s| s.to_str())
        {
            match fs::read_to_string(path) {
                Ok(xml) => {
                    // 解析 ID 并更新 id_map
                    if let Ok(style) = Style::from_xml(&xml) {
                        let id = match &style {
                            Style::Independent(s) => s.info.id.clone(),
                            Style::Dependent(s) => s.info.id.clone(),
                        };
                        self.id_map.insert(id, xml.clone());
                    }
                    self.styles.insert(name.to_string(), xml);
                    info!("热加载 CSL 样式: {name}");
                }
                Err(e) => {
                    error!("热加载 CSL 文件失败 {path:?}: {e}");
                }
            }
        }
    }
}

pub static REGISTRY: LazyLock<RwLock<StyleRegistry>> =
    LazyLock::new(|| RwLock::new(StyleRegistry::new()));
