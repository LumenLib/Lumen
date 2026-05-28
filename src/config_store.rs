use gpui::{App, Global};
use i18n::Language;
use log::info;

use crate::config::AppConfig;
use database::LocalStateManager;

pub struct ConfigStore {
    pub inner: AppConfig,
}

impl Global for ConfigStore {}

impl ConfigStore {
    pub fn current_language(&self) -> Language {
        self.ui.language.parse::<Language>().unwrap_or_default()
    }

    pub fn load_and_set(manager: &LocalStateManager, cx: &mut App) {
        let config = manager
            .load_config()
            .ok()
            .flatten()
            .and_then(|blob| {
                serde_json::from_str(&blob).ok().inspect(|_| {
                    info!("配置加载: 已从本地存储加载配置");
                })
            })
            .unwrap_or_else(|| {
                info!("配置加载: 未找到现有配置，使用默认配置");
                AppConfig::default()
            });
        cx.set_global(Self { inner: config });
    }
}

impl std::ops::Deref for ConfigStore {
    type Target = AppConfig;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
