//! 配置域类型（纯类型 + serde）
//!
//! 仅放置配置相关的数据结构、serde 实现与纯路径计算（`database_dir` /
//! `log_dir` 等）。平台相关与 IO 行为（目录解析、代理环境变量、目录创建、
//! 日志清理、默认配置构造）已统一上移到 `services::config`，本模块不依赖
//! 任何平台或 IO 逻辑。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_dir: PathBuf,
    pub attachment_path: PathBuf,
    pub filename_template: String,
    pub log_level: String, // "debug", "info", "warn", "error"
    #[serde(default)]
    pub notification_level: String, // "all", "warn", "error"
    pub ui: UiConfig,
    pub webdav: WebDavConfig,
    pub google_drive: GoogleDriveConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub pdf_viewer: PdfViewerConfig,
    #[serde(default)]
    pub translation: TranslationConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
}

/// 代理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// 是否启用代理
    pub enabled: bool,
    /// 代理地址，例如 http://127.0.0.1:7890 或 socks5://127.0.0.1:7890
    pub url: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: "http://127.0.0.1:7890".to_string(),
        }
    }
}

fn default_translation_font_size() -> f32 {
    14.0
}

/// PDF 查看器配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PdfViewerConfig {
    /// 是否使用自定义 PDF 打开方式
    pub use_custom: bool,
    /// macOS 上的自定义 PDF 打开程序路径
    pub macos_app: String,
    /// Windows 上的自定义 PDF 打开程序路径
    pub windows_app: String,
}

/// 翻译配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationConfig {
    pub engine: String,
    #[serde(default = "default_translation_font_size")]
    pub font_size: f32,
    #[serde(default = "default_target_language")]
    pub target_language: String,
}

fn default_target_language() -> String {
    "zh-CN".to_string()
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            engine: "google_free".to_string(),
            font_size: 14.0,
            target_language: "zh-CN".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub username: String,
    pub remote_path: String,
    /// 是否开启按需下载 (仅同步元数据，打开时下载)
    #[serde(default)]
    pub on_demand: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleDriveConfig {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
    #[serde(default)]
    pub authorized: bool,
}

fn default_ui_scale() -> f32 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme_mode: String,  // "light", "dark", "system"
    pub theme_style: String, // "default" or custom theme name
    pub language: String,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
}

/// 数据库配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub use_remote: bool,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub use_ssl: bool,
}

impl AppConfig {
    /// 获取数据库目录 (即 `base_dir`)
    #[must_use]
    pub fn database_dir(&self) -> PathBuf {
        self.base_dir.clone()
    }

    /// 获取日志目录
    #[must_use]
    pub fn log_dir(&self) -> PathBuf {
        self.base_dir.join("logs")
    }

    /// 获取主题目录
    #[must_use]
    pub fn themes_dir(&self) -> PathBuf {
        self.base_dir.join("themes")
    }

    /// 获取数据库文件的完整路径 (固定文件名)
    #[must_use]
    pub fn get_database_path(&self) -> PathBuf {
        self.base_dir.join("lumen.db")
    }

    /// 获取当前运行的日志文件路径 (基于时间)
    #[must_use]
    pub fn get_current_log_path(&self) -> PathBuf {
        let now = chrono::Local::now();
        let filename = format!("log_{}.log", now.format("%Y%m%d_%H%M%S"));
        self.log_dir().join(filename)
    }
}
