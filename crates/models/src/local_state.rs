use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowState {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub is_maximized: bool,
    #[serde(default)]
    pub is_fullscreen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppUiState {
    pub expanded_folder_ids: HashSet<String>,
    pub selected_sidebar_item: Option<String>,
    pub sort_field: Option<String>,
    pub sort_asc: bool,
    #[serde(default)]
    pub left_sidebar_width: Option<f64>,
    #[serde(default)]
    pub right_sidebar_width: Option<f64>,
    #[serde(default)]
    pub translation_keys: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub tags_sidebar_expanded: bool,
    #[serde(default = "default_true")]
    pub translation_original_expanded: bool,
    #[serde(default)]
    pub webdav_password: String,
    pub window_state: WindowState,
}

impl Default for AppUiState {
    fn default() -> Self {
        Self {
            expanded_folder_ids: HashSet::new(),
            selected_sidebar_item: None,
            sort_field: None,
            sort_asc: false,
            left_sidebar_width: None,
            right_sidebar_width: None,
            translation_keys: HashMap::new(),
            tags_sidebar_expanded: true,
            translation_original_expanded: true,
            webdav_password: String::new(),
            window_state: WindowState::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfState {
    pub id: String,
    pub path: String,
    pub page_index: u16,
    pub zoom_level: f32,
    pub offset_y: f32,
    pub fit_to_width: bool,
    pub auto_translate: bool,
    pub is_left_sidebar_open: bool,
    pub is_right_sidebar_open: bool,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
    pub last_read_at: u64,
}
