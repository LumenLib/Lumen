use serde::{Deserialize, Serialize};

/// WebDAV 连接配置（纯数据，serde）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebDavConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub username: String,
    pub password: String,
    pub remote_path: String,
}

impl Default for WebDavConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            username: String::new(),
            password: String::new(),
            remote_path: "/".to_string(),
        }
    }
}

/// Google Drive 连接配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleDriveConfig {
    pub enabled: bool,
    pub client_id: String,
    pub client_secret: String,
    pub refresh_token: String,
}
