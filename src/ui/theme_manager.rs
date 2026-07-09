use anyhow::Result;
use gpui::{Hsla, SharedString};
use gpui_component::select::SelectItem;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::{collections::HashMap, fs, path::Path, sync::RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeColors {
    pub background: String,
    pub foreground: String,
    pub primary: String,
    pub primary_foreground: String,
    pub secondary: String,
    pub muted: String,
    pub muted_foreground: String,
    pub accent: String,
    pub accent_foreground: String,
    pub border: String,
    pub input: String,
    pub popover: Option<String>,
    pub popover_foreground: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeScheme {
    pub name: String,
    pub light: ThemeColors,
    pub dark: ThemeColors,
}

impl ThemeColors {
    pub fn one_light() -> Self {
        Self {
            background: "#FAFAFA".to_string(),
            foreground: "#383A42".to_string(),
            primary: "#4078F2".to_string(),
            primary_foreground: "#FAFAFA".to_string(),
            secondary: "#F0F0F1".to_string(),
            muted: "#E5E5E6".to_string(),
            muted_foreground: "#8B8E96".to_string(),
            accent: "#4078F2".to_string(),
            accent_foreground: "#FFFFFF".to_string(),
            border: "#E5E5E6".to_string(),
            input: "#E5E5E6".to_string(),
            popover: Some("#E5E5E6".to_string()),
            popover_foreground: Some("#383A42".to_string()),
        }
    }

    pub fn one_dark() -> Self {
        Self {
            background: "#282C34".to_string(),
            foreground: "#ABB2BF".to_string(),
            primary: "#61AFEF".to_string(),
            primary_foreground: "#282C34".to_string(),
            secondary: "#21252B".to_string(),
            muted: "#3E4452".to_string(),
            muted_foreground: "#6B7280".to_string(),
            accent: "#61AFEF".to_string(),
            accent_foreground: "#282C34".to_string(),
            border: "#181A1F".to_string(),
            input: "#3E4452".to_string(),
            popover: Some("#3E4452".to_string()),
            popover_foreground: Some("#ABB2BF".to_string()),
        }
    }

    pub fn apply_to_palette(&self, theme: &mut gpui_component::Theme) {
        let bg = self.parse_hex(&self.background);
        let fg = self.parse_hex(&self.foreground);

        theme.background = bg;
        theme.foreground = fg;
        theme.primary = self.parse_hex(&self.primary);
        theme.primary_foreground = self.parse_hex(&self.primary_foreground);
        theme.secondary = self.parse_hex(&self.secondary);
        theme.muted = self.parse_hex(&self.muted);
        theme.muted_foreground = self.parse_hex(&self.muted_foreground);
        theme.accent = self.parse_hex(&self.accent);
        theme.accent_foreground = self.parse_hex(&self.accent_foreground);
        theme.border = self.parse_hex(&self.border);
        theme.input = self.parse_hex(&self.input);

        // 处理 popover 颜色，如果未配置则跟随 background/foreground
        theme.popover = self.popover.as_ref().map_or(bg, |c| self.parse_hex(c));
        theme.popover_foreground = self
            .popover_foreground
            .as_ref()
            .map_or(fg, |c| self.parse_hex(c));
    }

    pub fn parse_hex(&self, hex: &str) -> Hsla {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            gpui::rgb(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)).into()
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255);
            gpui::rgba(u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b) | u32::from(a)).into()
        } else {
            gpui::black()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSelectItem {
    pub id: String,
    pub label: String,
}

impl SelectItem for ThemeSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

impl Default for ThemeLoader {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ThemeLoader {
    themes: HashMap<String, ThemeScheme>,
}

impl ThemeLoader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            themes: HashMap::new(),
        }
    }

    pub fn load_all(&mut self, themes_dir: &Path) -> Result<()> {
        if !themes_dir.exists() {
            let _ = fs::create_dir_all(themes_dir);
            return Ok(());
        }

        for entry in fs::read_dir(themes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                if let Ok(scheme) = serde_json::from_str::<ThemeScheme>(&content) {
                    log::info!("加载自定义主题: {}", scheme.name);
                    self.themes.insert(scheme.name.clone(), scheme);
                }
            }
        }
        Ok(())
    }

    pub fn load_from_string(&mut self, content: &str) -> Result<()> {
        if let Ok(scheme) = serde_json::from_str::<ThemeScheme>(content) {
            log::info!("加载内置主题: {}", scheme.name);
            self.themes.insert(scheme.name.clone(), scheme);
        }
        Ok(())
    }

    #[must_use]
    pub fn get_theme(&self, name: &str) -> Option<&ThemeScheme> {
        self.themes.get(name)
    }

    #[must_use]
    pub fn available_themes(&self) -> Vec<String> {
        let mut names: Vec<String> = self.themes.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn reload_theme_from_file(&mut self, path: &Path) -> Result<()> {
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(path)?;
            if let Ok(scheme) = serde_json::from_str::<ThemeScheme>(&content) {
                log::info!("热加载主题: {}", scheme.name);
                self.themes.insert(scheme.name.clone(), scheme);
            }
        }
        Ok(())
    }
}

pub static LOADER: LazyLock<RwLock<ThemeLoader>> =
    LazyLock::new(|| RwLock::new(ThemeLoader::new()));
