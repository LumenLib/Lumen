//! 配置读写的服务层编排
//!
//! 职责：把“从 `LocalStateManager` 读取/写入配置 JSON”的原始 CRUD，封装为带
//! 解析、默认值回落、序列化错误处理的服务层操作。UI 与启动路径只调用本模块，
//! 不直接碰 `database` 的 `load_config` / `save_config`。

use database::LocalStateManager;
use models::config::AppConfig;

/// 从本地状态库加载配置；不存在或解析失败时回落默认配置，并把默认配置写回。
#[must_use]
pub fn load_config(manager: &LocalStateManager) -> AppConfig {
    manager
        .load_config()
        .ok()
        .flatten()
        .and_then(|blob| serde_json::from_str(&blob).ok())
        .unwrap_or_else(|| {
            let default = AppConfig::default();
            if let Ok(blob) = serde_json::to_string(&default) {
                let _ = manager.save_config(&blob);
            }
            default
        })
}

/// 将配置序列化并写入本地状态库。
pub fn save_config(manager: &LocalStateManager, config: &AppConfig) -> anyhow::Result<()> {
    let blob = serde_json::to_string(config)?;
    manager.save_config(&blob)?;
    Ok(())
}
