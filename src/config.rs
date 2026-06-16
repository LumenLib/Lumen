use anyhow::{Context, Result};
use database::DatabaseConfig;
use log::debug;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[cfg(target_os = "windows")]
pub fn get_app_root_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(|p| PathBuf::from(p).join("Lumen"))
        .unwrap_or_else(|_| {
            std::env::var("USERPROFILE")
                .map(|p| PathBuf::from(p).join(".Lumen"))
                .unwrap_or_else(|_| PathBuf::from(".Lumen"))
        })
}

#[cfg(target_os = "macos")]
pub fn get_app_root_dir() -> PathBuf {
    std::env::var("HOME")
        .map(|p| PathBuf::from(p).join("Library/Application Support/Lumen"))
        .unwrap_or_else(|_| PathBuf::from(".Lumen"))
}

#[cfg(target_os = "linux")]
pub fn get_app_root_dir() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(|p| PathBuf::from(p).join("Lumen"))
        .or_else(|_| std::env::var("HOME").map(|p| PathBuf::from(p).join(".config/Lumen")))
        .unwrap_or_else(|_| PathBuf::from(".Lumen"))
}

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

/// 应用代理配置到当前进程的环境变量中
pub fn apply_proxy_config(config: &ProxyConfig) {
    if config.enabled && !config.url.trim().is_empty() {
        let url = config.url.trim();
        unsafe {
            std::env::set_var("HTTP_PROXY", url);
            std::env::set_var("HTTPS_PROXY", url);
            std::env::set_var("ALL_PROXY", url); // 同时支持 socks 代理等
        }
        log::info!("自定义代理已启用并在当前进程生效: {}", url);
    } else {
        unsafe {
            std::env::remove_var("HTTP_PROXY");
            std::env::remove_var("HTTPS_PROXY");
            std::env::remove_var("ALL_PROXY");
        }
        log::info!("自定义代理已从当前进程禁用（恢复系统默认网络环境）");
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

impl Default for AppConfig {
    fn default() -> Self {
        let base_dir = get_app_root_dir();
        Self {
            attachment_path: base_dir.join("storage"),
            base_dir,
            filename_template: "{author}{year}-{title}".to_string(),
            log_level: "info".to_string(),
            notification_level: "all".to_string(),
            ui: UiConfig {
                theme_mode: "light".to_string(),
                theme_style: "default".to_string(),
                language: "zh-CN".to_string(),
                ui_scale: 1.0,
            },
            webdav: WebDavConfig {
                enabled: false,
                endpoint: String::new(),
                username: String::new(),
                remote_path: "/".to_string(),
                on_demand: false,
            },
            google_drive: GoogleDriveConfig::default(),
            database: DatabaseConfig {
                use_remote: false,
                host: "localhost".to_string(),
                port: 3306,
                database: "lumen".to_string(),
                username: "root".to_string(),
                password: String::new(),
                use_ssl: false,
            },
            pdf_viewer: PdfViewerConfig::default(),
            translation: TranslationConfig::default(),
            proxy: ProxyConfig::default(),
        }
    }
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

    /// 获取 CSL 样式目录
    #[must_use]
    pub fn csl_dir(&self) -> PathBuf {
        self.base_dir.join("styles")
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

    /// 确保配置中定义的所有物理目录都已创建
    pub fn ensure_dirs(&self) -> Result<()> {
        let dirs = [
            self.attachment_path.clone(),
            self.base_dir.clone(),
            self.log_dir(),
            self.csl_dir(),
            self.themes_dir(),
        ];
        for dir in dirs {
            if !dir.exists() {
                fs::create_dir_all(&dir)
                    .with_context(|| format!("Failed to create directory: {dir:?}"))?;
            }
        }
        Ok(())
    }

    /// 清理旧日志文件，只保留最近的 20 个
    pub fn clean_old_logs(&self) {
        const MAX_LOG_FILES: usize = 20;
        let log_dir = self.log_dir();

        if let Ok(entries) = fs::read_dir(&log_dir) {
            let mut log_files: Vec<PathBuf> = entries
                .filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|s| s.starts_with("log_") && s.ends_with(".log"))
                })
                .collect();

            // 按文件名排序 (因为文件名包含时间戳 YYYYMMDD_HHMMSS，所以字典序即时间序)
            log_files.sort();

            // 如果超过限制，删除最旧的
            if log_files.len() > MAX_LOG_FILES {
                let to_remove = log_files.len() - MAX_LOG_FILES;
                debug!(
                    "日志清理: 需删除 {} 个旧日志文件 (共 {} 个, 上限 {})",
                    to_remove,
                    log_files.len(),
                    MAX_LOG_FILES
                );
                for path in log_files.iter().take(to_remove) {
                    if let Err(e) = fs::remove_file(path) {
                        debug!("日志清理: 删除旧日志文件失败 {path:?}: {e}");
                    } else {
                        debug!("日志清理: 已删除旧日志文件 {path:?}");
                    }
                }
            }
        }
    }
}
