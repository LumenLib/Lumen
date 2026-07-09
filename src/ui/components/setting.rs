use crate::RUNTIME;
use crate::config::AppConfig;
use crate::config_store::ConfigStore;
use crate::notification_bus::{NotificationType, show_notification};
use crate::services::{MainApp, utils::filename};
use crate::ui::{
    components::ToastOverlay,
    icons::IconName,
    theme_manager::{LOADER, ThemeSelectItem},
};
#[cfg(not(target_os = "macos"))]
use gpui::WindowControlArea;
use gpui::prelude::*;
use gpui::{
    App, AppContext, AsyncApp, DefiniteLength, Div, Entity, EntityInputHandler, FontWeight,
    MouseButton, PathPromptOptions, Result, SharedString, WeakEntity, Window, div, rems,
    transparent_black,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, Theme,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement,
    select::{Select, SelectDelegate, SelectEvent, SelectItem, SelectState},
    switch::Switch,
    v_flex,
};

fn setting_input(input: Input, theme: &Theme) -> Div {
    div()
        .bg(theme.muted)
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(input.appearance(false))
}

fn setting_select<D: SelectDelegate + 'static>(select: Select<D>, theme: &Theme) -> Div {
    div()
        .bg(theme.muted)
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(select.appearance(false))
}
use i18n::{
    Language, {I18nKey, t},
};
use log::{debug, error, info};
use std::sync::Arc;

/// 设置页面分类
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsTab {
    General,
    Sync,
    AiBackends,
    Translation,
    AiChat,
    About,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLevelItem {
    pub value: String,
    pub label: String,
}

impl SelectItem for LogLevelItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationLevelItem {
    pub value: String,
    pub label: String,
}

impl SelectItem for NotificationLevelItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScaleItem {
    pub value: f32,
    pub label: String,
}

impl SelectItem for ScaleItem {
    type Value = f32;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationEngineItem {
    pub value: String,
    pub label: String,
}

impl SelectItem for TranslationEngineItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &String {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendKindItem {
    pub value: String,
    pub label: &'static str,
}

impl SelectItem for BackendKindItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
pub struct CompressionStrategyItem {
    pub value: String,
    pub label: String,
}

impl SelectItem for CompressionStrategyItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

#[derive(Clone)]
pub struct LanguageItem {
    pub value: String,
    pub label: String,
}

impl SelectItem for LanguageItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &String {
        &self.value
    }
}

/// 同步设置子标签
#[derive(Debug, Clone, Copy, PartialEq)]
enum SyncSubTab {
    Metadata,
    Attachment,
}

/// 设置窗口视图
pub struct SettingsWindow {
    app: Arc<MainApp>,
    config: AppConfig,
    initial_config: AppConfig,
    active_tab: SettingsTab,

    // 下拉框状态
    language_select: Entity<SelectState<Vec<Language>>>,
    theme_style_select: Entity<SelectState<Vec<ThemeSelectItem>>>,
    ui_scale_select: Entity<SelectState<Vec<ScaleItem>>>,
    log_level_select: Entity<SelectState<Vec<LogLevelItem>>>,
    notification_level_select: Entity<SelectState<Vec<NotificationLevelItem>>>,
    // 输入框状态
    attachment_path_input: Entity<InputState>,
    base_dir_input: Entity<InputState>,
    filename_template_input: Entity<InputState>,

    // WebDAV 设置
    webdav_enabled: bool,
    webdav_on_demand: bool,
    webdav_endpoint_input: Entity<InputState>,
    webdav_username_input: Entity<InputState>,
    webdav_password_input: Entity<InputState>,
    webdav_remote_path_input: Entity<InputState>,

    // 同步子标签
    sync_sub_tab: SyncSubTab,

    // Google Drive 设置
    google_drive_enabled: bool,
    google_drive_authorized: bool,
    google_drive_client_id_input: Entity<InputState>,
    google_drive_client_secret_input: Entity<InputState>,

    // 数据库设置
    db_use_remote: bool,
    db_host_input: Entity<InputState>,
    db_port_input: Entity<InputState>,
    db_name_input: Entity<InputState>,
    db_user_input: Entity<InputState>,
    db_pass_input: Entity<InputState>,
    db_use_ssl: bool,

    // PDF 阅读器设置
    pdf_use_custom: bool,
    pdf_macos_app_input: Entity<InputState>,
    pdf_windows_app_input: Entity<InputState>,

    // 翻译设置
    translation_engine_select: Entity<SelectState<Vec<TranslationEngineItem>>>,
    target_language_select: Entity<SelectState<Vec<LanguageItem>>>,
    google_api_key_input: Entity<InputState>,
    niutrans_api_key_input: Entity<InputState>,
    baidu_api_key_input: Entity<InputState>,
    youdao_api_key_input: Entity<InputState>,
    deepl_api_key_input: Entity<InputState>,
    ai_entries: Vec<translate::AiBackendEntry>,
    ai_active_name: String,
    ai_edit_target: Option<usize>,
    ai_adding_new: bool,
    ai_edit_name_input: Entity<InputState>,
    ai_edit_kind_select: Entity<SelectState<Vec<BackendKindItem>>>,
    ai_edit_kind_value: String,
    ai_edit_api_key_input: Entity<InputState>,
    ai_edit_api_base_input: Entity<InputState>,
    ai_edit_model_input: Entity<InputState>,
    ai_edit_context_window_input: Entity<InputState>,
    ai_edit_compression_strategy_select: Entity<SelectState<Vec<CompressionStrategyItem>>>,
    ai_edit_compression_strategy_value: String,
    ai_edit_enable_thinking: bool,
    // AI Chat 设置
    chat_active_name: String,
    chat_default_system_prompt_input: Entity<InputState>,

    // 测试状态
    webdav_tested: bool,
    db_tested: bool,
    webdav_test_result: Option<Result<(), String>>,
    db_test_result: Option<Result<(), String>>,
    // EasyScholar
    easyscholar_key_input: Entity<InputState>,

    // Network Proxy
    proxy_enabled: bool,
    proxy_url_input: Entity<InputState>,

    saved_flag: Arc<std::sync::atomic::AtomicBool>,
    #[allow(dead_code)]
    close_subscription: Option<gpui::Subscription>,
    toast_overlay: Entity<ToastOverlay>,
}

impl SettingsWindow {
    pub fn new(
        app: Arc<MainApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
        initial_tab: Option<SettingsTab>,
    ) -> Self {
        // 读取当前配置的副本
        let config = app.config.lock().expect("Failed to lock AppConfig").clone();
        let initial_config = config.clone();

        // 1. 语言选择
        let languages = vec![
            Language::ZhCn,
            Language::ZhTw,
            Language::En,
            Language::Ja,
            Language::Ko,
            Language::Ru,
            Language::Fr,
            Language::De,
            Language::Es,
        ];
        let current_lang = config.ui.language.parse::<Language>().unwrap_or_default();
        let language_select = cx.new(|cx| {
            let mut state = SelectState::new(languages, None, window, cx);
            state.set_selected_value(&current_lang, window, cx);
            state
        });
        cx.subscribe(&language_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(lang)) = event {
                this.config.ui.language = lang.as_str().to_string();
                this.apply_temporary_config(cx);
            }
        })
        .detach();

        // 2. 主题方案选择
        let theme_styles = {
            let loader = LOADER.read().ok();
            let mut items = vec![ThemeSelectItem {
                id: "default".to_string(),
                label: "Default".to_string(),
            }];
            if let Some(loader) = loader {
                for name in loader.available_themes() {
                    items.push(ThemeSelectItem {
                        id: name.clone(),
                        label: name,
                    });
                }
            }
            items
        };
        let current_style = config.ui.theme_style.clone();
        let theme_style_select = cx.new(|cx| {
            let mut state = SelectState::new(theme_styles, None, window, cx);
            state.set_selected_value(&current_style, window, cx);
            state
        });
        cx.subscribe(&theme_style_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(style_id)) = event {
                this.set_theme_style(style_id, cx);
            }
        })
        .detach();

        // 2.05 UI 缩放
        let scale_items = vec![
            ScaleItem {
                value: 0.8,
                label: "80%".to_string(),
            },
            ScaleItem {
                value: 0.9,
                label: "90%".to_string(),
            },
            ScaleItem {
                value: 1.0,
                label: "100%".to_string(),
            },
            ScaleItem {
                value: 1.1,
                label: "110%".to_string(),
            },
            ScaleItem {
                value: 1.25,
                label: "125%".to_string(),
            },
            ScaleItem {
                value: 1.5,
                label: "150%".to_string(),
            },
            ScaleItem {
                value: 1.75,
                label: "175%".to_string(),
            },
            ScaleItem {
                value: 2.0,
                label: "200%".to_string(),
            },
        ];
        let current_scale = config.ui.ui_scale;
        let ui_scale_select = cx.new(|cx| {
            let mut state = SelectState::new(scale_items, None, window, cx);
            state.set_selected_value(&current_scale, window, cx);
            state
        });
        cx.subscribe(&ui_scale_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(scale)) = event {
                info!("UI 缩放比例已调整为: {scale}");
                this.config.ui.ui_scale = *scale;
                this.apply_temporary_config(cx);
            }
        })
        .detach();

        // 2.1 日志等级选择
        let log_levels = vec![
            LogLevelItem {
                value: "debug".to_string(),
                label: "Debug".to_string(),
            },
            LogLevelItem {
                value: "info".to_string(),
                label: "Info".to_string(),
            },
            LogLevelItem {
                value: "warn".to_string(),
                label: "Warn".to_string(),
            },
            LogLevelItem {
                value: "error".to_string(),
                label: "Error".to_string(),
            },
        ];
        let current_log_level = config.log_level.clone();
        let log_level_select = cx.new(|cx| {
            let mut state = SelectState::new(log_levels, None, window, cx);
            state.set_selected_value(&current_log_level, window, cx);
            state
        });
        cx.subscribe(&log_level_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(level)) = event {
                this.config.log_level = level.clone();
                this.apply_temporary_config(cx);
                if let Ok(lf) = level.parse::<log::LevelFilter>() {
                    log::set_max_level(lf);
                }
            }
        })
        .detach();

        // 2.2 通知层级选择
        let notification_levels = vec![
            NotificationLevelItem {
                value: "all".to_string(),
                label: "All".to_string(),
            },
            NotificationLevelItem {
                value: "warn".to_string(),
                label: "Warn".to_string(),
            },
            NotificationLevelItem {
                value: "error".to_string(),
                label: "Error".to_string(),
            },
        ];
        let current_notification_level = config.notification_level.clone();
        let notification_level_select = cx.new(|cx| {
            let mut state = SelectState::new(notification_levels, None, window, cx);
            state.set_selected_value(&current_notification_level, window, cx);
            state
        });
        cx.subscribe(&notification_level_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(level)) = event {
                this.config.notification_level = level.clone();
                this.apply_temporary_config(cx);
            }
        })
        .detach();

        // 3. 目录与路径
        let attachment_path_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.attachment_path.to_string_lossy().to_string())
        });
        let base_dir_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(config.base_dir.to_string_lossy().to_string())
        });
        let filename_template_input = cx
            .new(|cx| InputState::new(window, cx).default_value(config.filename_template.clone()));

        // 4. WebDAV
        let webdav_endpoint_input =
            cx.new(|cx| InputState::new(window, cx).default_value(config.webdav.endpoint.clone()));
        let webdav_username_input =
            cx.new(|cx| InputState::new(window, cx).default_value(config.webdav.username.clone()));
        let webdav_pw = app.local_state.read().unwrap().webdav_password.clone();
        let webdav_password_input =
            cx.new(|cx| InputState::new(window, cx).default_value(webdav_pw));
        let webdav_remote_path_input = cx
            .new(|cx| InputState::new(window, cx).default_value(config.webdav.remote_path.clone()));

        // 4.5. Google Drive
        let google_drive_client_id_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(config.google_drive.client_id.clone())
        });
        let google_drive_client_secret_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(config.google_drive.client_secret.clone())
        });

        // 5. 数据库
        let db_host_input =
            cx.new(|cx| InputState::new(window, cx).default_value(config.database.host.clone()));
        let db_port_input = cx
            .new(|cx| InputState::new(window, cx).default_value(config.database.port.to_string()));
        let db_name_input = cx
            .new(|cx| InputState::new(window, cx).default_value(config.database.database.clone()));
        let db_user_input = cx
            .new(|cx| InputState::new(window, cx).default_value(config.database.username.clone()));
        let db_pass_input = cx
            .new(|cx| InputState::new(window, cx).default_value(config.database.password.clone()));

        // 6. PDF 阅读器
        let pdf_macos_app_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(config.pdf_viewer.macos_app.clone())
        });
        let pdf_windows_app_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(config.pdf_viewer.windows_app.clone())
        });

        // 7. EasyScholar Key
        let easyscholar_key_val = app
            .sync_service
            .db
            .get_sync_meta("easyscholar_key")
            .ok()
            .flatten()
            .unwrap_or_default();
        let easyscholar_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(easyscholar_key_val)
                .placeholder(t(I18nKey::EasyScholarPlaceholder, current_lang))
        });

        // Network Proxy
        let proxy_enabled = config.proxy.enabled;
        let proxy_url_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(config.proxy.url.clone())
                .placeholder(t(I18nKey::ProxyDesc, current_lang))
        });

        cx.subscribe(
            &filename_template_input,
            |_, _, _: &gpui_component::input::InputEvent, cx| {
                cx.notify();
            },
        )
        .detach();

        let saved_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let saved_flag_for_close = saved_flag.clone();

        let initial_config_for_close = config.clone();
        let app_for_close = app.clone();

        let close_subscription = cx.on_window_closed(move |cx| {
            if !saved_flag_for_close.load(std::sync::atomic::Ordering::SeqCst) {
                info!("设置窗口未保存即关闭，正在还原配置和主题...");
                // 1. 还原 AppConfig
                if let Ok(mut config) = app_for_close.config.lock() {
                    *config = initial_config_for_close.clone();
                }

                // 2. 还原 ConfigStore (将自动触发 MainWindow 的观察者进行视觉回滚)
                cx.set_global(ConfigStore {
                    inner: initial_config_for_close.clone(),
                });
            }
        });

        // 8. 翻译设置
        let lang = config.ui.language.parse::<Language>().unwrap_or_default();
        let engine_items: Vec<_> = translate::ENGINES
            .iter()
            .map(|e| {
                let label = match e.id {
                    "google_free" => t(I18nKey::EngineGoogleFree, lang),
                    "bing_free" => t(I18nKey::EngineBingFree, lang),
                    "google" => t(I18nKey::EngineGoogleCloud, lang),
                    "niutrans" => t(I18nKey::EngineNiuTrans, lang),
                    "baidu" => t(I18nKey::EngineBaidu, lang),
                    "youdao" => t(I18nKey::EngineYoudao, lang),
                    "deepl_free" => t(I18nKey::EngineDeeplFree, lang),
                    "deepl_pro" => t(I18nKey::EngineDeeplPro, lang),
                    "ai" => t(I18nKey::EngineAi, lang),
                    _ => t(I18nKey::EngineGoogleFree, lang),
                };
                TranslationEngineItem {
                    value: e.id.to_string(),
                    label: label.to_string(),
                }
            })
            .collect();
        let current_engine = config.translation.engine.clone();
        let translation_engine_select = cx.new(|cx| {
            let mut state = SelectState::new(engine_items, None, window, cx);
            state.set_selected_value(&current_engine, window, cx);
            state
        });
        cx.subscribe(&translation_engine_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(engine)) = event {
                this.config.translation.engine = engine.clone();
                this.apply_temporary_config(cx);
            }
        })
        .detach();

        let language_items = vec![
            LanguageItem {
                value: "zh-CN".into(),
                label: "简体中文".into(),
            },
            LanguageItem {
                value: "zh-TW".into(),
                label: "繁體中文".into(),
            },
            LanguageItem {
                value: "en".into(),
                label: "English".into(),
            },
            LanguageItem {
                value: "ja".into(),
                label: "日本語".into(),
            },
            LanguageItem {
                value: "ko".into(),
                label: "한국어".into(),
            },
            LanguageItem {
                value: "fr".into(),
                label: "Français".into(),
            },
            LanguageItem {
                value: "de".into(),
                label: "Deutsch".into(),
            },
            LanguageItem {
                value: "es".into(),
                label: "Español".into(),
            },
            LanguageItem {
                value: "ru".into(),
                label: "Русский".into(),
            },
            LanguageItem {
                value: "pt".into(),
                label: "Português".into(),
            },
            LanguageItem {
                value: "it".into(),
                label: "Italiano".into(),
            },
            LanguageItem {
                value: "nl".into(),
                label: "Nederlands".into(),
            },
            LanguageItem {
                value: "ar".into(),
                label: "العربية".into(),
            },
            LanguageItem {
                value: "th".into(),
                label: "ไทย".into(),
            },
            LanguageItem {
                value: "vi".into(),
                label: "Tiếng Việt".into(),
            },
        ];
        let current_lang = config.translation.target_language.clone();
        let target_language_select = cx.new(|cx| {
            let mut state = SelectState::new(language_items, None, window, cx);
            state.set_selected_value(&current_lang, window, cx);
            state
        });
        cx.subscribe(&target_language_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(lang)) = event {
                this.config.translation.target_language = lang.clone();
                this.apply_temporary_config(cx);
            }
        })
        .detach();

        let translation_keys = app.local_state.read().unwrap().translation_keys.clone();
        let google_api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(translation_keys.get("google").cloned().unwrap_or_default())
        });
        let niutrans_api_key_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(
                translation_keys
                    .get("niutrans")
                    .cloned()
                    .unwrap_or_default(),
            )
        });
        let baidu_api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(translation_keys.get("baidu").cloned().unwrap_or_default())
        });
        let deepl_api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(translation_keys.get("deepl").cloned().unwrap_or_default())
        });
        let youdao_api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(translation_keys.get("youdao").cloned().unwrap_or_default())
        });
        let ai_entries: Vec<translate::AiBackendEntry> = translation_keys
            .get("ai.entries")
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let ai_active_name = translation_keys
            .get("ai.active")
            .cloned()
            .unwrap_or_default();
        let ai_edit_name_input = cx.new(|cx| InputState::new(window, cx));
        let backend_kinds = vec![
            BackendKindItem {
                value: "openai".into(),
                label: "OpenAI",
            },
            BackendKindItem {
                value: "ollama".into(),
                label: "Ollama",
            },
            BackendKindItem {
                value: "claude".into(),
                label: "Claude",
            },
            BackendKindItem {
                value: "siliconflow".into(),
                label: "SiliconFlow",
            },
        ];
        let ai_edit_kind_select = cx.new(|cx| {
            let mut state = SelectState::new(backend_kinds, None, window, cx);
            state.set_selected_value(&"openai".to_string(), window, cx);
            state
        });
        let ai_edit_kind_value = "openai".to_string();
        cx.subscribe(&ai_edit_kind_select, {
            move |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(kind)) = event {
                    this.ai_edit_kind_value = kind.clone();
                    cx.notify();
                }
            }
        })
        .detach();
        let ai_edit_api_key_input = cx.new(|cx| InputState::new(window, cx).masked(true));
        let ai_edit_api_base_input = cx.new(|cx| InputState::new(window, cx));
        let ai_edit_model_input = cx.new(|cx| InputState::new(window, cx));
        let ai_edit_context_window_input = cx.new(|cx| InputState::new(window, cx));

        let compression_strategies = vec![
            CompressionStrategyItem {
                value: "sliding_window".into(),
                label: t(I18nKey::SlidingWindow, lang).to_string(),
            },
            CompressionStrategyItem {
                value: "summary".into(),
                label: t(I18nKey::SummaryCompression, lang).to_string(),
            },
        ];
        let ai_edit_compression_strategy_select = cx.new(|cx| {
            let mut state = SelectState::new(compression_strategies, None, window, cx);
            state.set_selected_value(&"sliding_window".to_string(), window, cx);
            state
        });
        let ai_edit_compression_strategy_value = "sliding_window".to_string();
        cx.subscribe(&ai_edit_compression_strategy_select, {
            move |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(val)) = event {
                    this.ai_edit_compression_strategy_value = val.clone();
                    cx.notify();
                }
            }
        })
        .detach();

        let chat_active_name = translation_keys
            .get("chat.active")
            .cloned()
            .unwrap_or_default();
        let chat_default_system_prompt_val = translation_keys
            .get("chat.default_system_prompt")
            .cloned()
            .unwrap_or_default();
        let chat_default_system_prompt_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(chat_default_system_prompt_val)
                .multi_line(true)
        });

        let toast_overlay = cx.new(|cx| ToastOverlay::new(window, cx));

        Self {
            app,
            initial_config,
            webdav_enabled: config.webdav.enabled,
            webdav_on_demand: config.webdav.on_demand,
            db_use_remote: config.database.use_remote,
            db_use_ssl: config.database.use_ssl,
            pdf_use_custom: config.pdf_viewer.use_custom,
            sync_sub_tab: SyncSubTab::Metadata,
            google_drive_enabled: config.google_drive.enabled,
            google_drive_authorized: config.google_drive.authorized,
            config,
            active_tab: initial_tab.unwrap_or(SettingsTab::General),
            language_select,
            theme_style_select,
            ui_scale_select,
            log_level_select,
            notification_level_select,
            attachment_path_input,
            base_dir_input,
            filename_template_input,
            webdav_endpoint_input,
            webdav_username_input,
            webdav_password_input,
            webdav_remote_path_input,
            google_drive_client_id_input,
            google_drive_client_secret_input,
            db_host_input,
            db_port_input,
            db_name_input,
            db_user_input,
            db_pass_input,
            pdf_macos_app_input,
            pdf_windows_app_input,
            translation_engine_select,
            target_language_select,
            google_api_key_input,
            niutrans_api_key_input,
            baidu_api_key_input,
            youdao_api_key_input,
            deepl_api_key_input,
            ai_entries,
            ai_active_name,
            ai_edit_target: None,
            ai_adding_new: false,
            ai_edit_name_input,
            ai_edit_kind_select,
            ai_edit_kind_value,
            ai_edit_api_key_input,
            ai_edit_api_base_input,
            ai_edit_model_input,
            ai_edit_context_window_input,
            ai_edit_compression_strategy_select,
            ai_edit_compression_strategy_value,
            ai_edit_enable_thinking: false,
            chat_active_name,
            chat_default_system_prompt_input,
            webdav_tested: false,
            db_tested: false,
            webdav_test_result: None,
            db_test_result: None,
            easyscholar_key_input,
            proxy_enabled,
            proxy_url_input,
            saved_flag,
            close_subscription: Some(close_subscription),
            toast_overlay,
        }
    }

    fn handle_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("设置窗口: 开始保存配置");
        self.saved_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let mut new_config = self.config.clone();
        new_config.attachment_path = self
            .attachment_path_input
            .read(cx)
            .text()
            .to_string()
            .into();
        new_config.base_dir = self.base_dir_input.read(cx).text().to_string().into();
        new_config.filename_template = self.filename_template_input.read(cx).text().to_string();

        // WebDAV
        new_config.webdav.enabled = self.webdav_enabled;
        new_config.webdav.on_demand = self.webdav_on_demand;
        new_config.webdav.endpoint = self.webdav_endpoint_input.read(cx).text().to_string();
        new_config.webdav.username = self.webdav_username_input.read(cx).text().to_string();
        new_config.webdav.remote_path = self.webdav_remote_path_input.read(cx).text().to_string();

        // Google Drive
        new_config.google_drive.enabled = self.google_drive_enabled;
        new_config.google_drive.client_id = self
            .google_drive_client_id_input
            .read(cx)
            .text()
            .to_string();
        new_config.google_drive.client_secret = self
            .google_drive_client_secret_input
            .read(cx)
            .text()
            .to_string();
        new_config.google_drive.authorized = self.google_drive_authorized;

        // 数据库
        new_config.database.use_remote = self.db_use_remote;
        new_config.database.host = self.db_host_input.read(cx).text().to_string();
        new_config.database.port = self
            .db_port_input
            .read(cx)
            .text()
            .to_string()
            .parse()
            .unwrap_or(3306);
        new_config.database.database = self.db_name_input.read(cx).text().to_string();
        new_config.database.username = self.db_user_input.read(cx).text().to_string();
        new_config.database.password = self.db_pass_input.read(cx).text().to_string();
        new_config.database.use_ssl = self.db_use_ssl;

        // PDF 阅读器
        new_config.pdf_viewer.use_custom = self.pdf_use_custom;
        new_config.pdf_viewer.macos_app = self.pdf_macos_app_input.read(cx).text().to_string();
        new_config.pdf_viewer.windows_app = self.pdf_windows_app_input.read(cx).text().to_string();

        // 代理
        new_config.proxy.enabled = self.proxy_enabled;
        new_config.proxy.url = self.proxy_url_input.read(cx).text().to_string();

        // 翻译
        let google_key = self.google_api_key_input.read(cx).text().to_string();
        let niutrans_key = self.niutrans_api_key_input.read(cx).text().to_string();
        let baidu_key = self.baidu_api_key_input.read(cx).text().to_string();
        let youdao_key = self.youdao_api_key_input.read(cx).text().to_string();
        let deepl_key = self.deepl_api_key_input.read(cx).text().to_string();
        let webdav_password = self.webdav_password_input.read(cx).text().to_string();
        if let Ok(mut state) = self.app.local_state.write() {
            state
                .translation_keys
                .insert("google".to_string(), google_key);
            state
                .translation_keys
                .insert("niutrans".to_string(), niutrans_key);
            state
                .translation_keys
                .insert("baidu".to_string(), baidu_key);
            state
                .translation_keys
                .insert("youdao".to_string(), youdao_key);
            state
                .translation_keys
                .insert("deepl".to_string(), deepl_key);
            state.translation_keys.insert(
                "ai.entries".to_string(),
                serde_json::to_string(&self.ai_entries).unwrap_or_default(),
            );
            state
                .translation_keys
                .insert("ai.active".to_string(), self.ai_active_name.clone());
            state
                .translation_keys
                .insert("chat.active".to_string(), self.chat_active_name.clone());
            state.translation_keys.insert(
                "chat.default_system_prompt".to_string(),
                self.chat_default_system_prompt_input
                    .read(cx)
                    .text()
                    .to_string(),
            );
            state.webdav_password = webdav_password;
            let _ = self.app.local_state_manager.save_all(&state);
        }

        // 保存 EasyScholar Key
        let es_key = self.easyscholar_key_input.read(cx).text().to_string();
        // 保存到数据库 (同步元数据)
        let _ = self
            .app
            .sync_service
            .db
            .set_sync_meta("easyscholar_key", &es_key);

        // 更新 ConfigStore Global（子视图将在下次 render 时读取）
        cx.set_global(ConfigStore {
            inner: new_config.clone(),
        });

        // 通知所有 UI 刷新
        self.app.notify_ui_changed();

        if let Err(e) = self.app.update_config(new_config) {
            error!("更新配置失败: {e}");
        } else {
            info!("设置窗口: 配置已保存");
            window.remove_window();
        }
    }

    fn apply_temporary_config(&self, cx: &mut App) {
        if let Ok(mut config) = self.app.config.lock() {
            *config = self.config.clone();
        }
        cx.set_global(ConfigStore {
            inner: self.config.clone(),
        });
    }

    fn handle_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("设置窗口: 取消并还原配置");
        self.saved_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.config = self.initial_config.clone();
        self.apply_temporary_config(cx);
        window.remove_window();
    }

    fn set_theme_mode(&mut self, mode: &str, cx: &mut Context<Self>) {
        self.config.ui.theme_mode = mode.to_string();
        self.apply_temporary_config(cx);
    }

    fn set_theme_style(&mut self, style: &str, cx: &mut Context<Self>) {
        self.config.ui.theme_style = style.to_string();
        self.apply_temporary_config(cx);
    }

    fn render_sidebar_item(
        &self,
        tab: SettingsTab,
        icon: impl IntoElement,
        label: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.active_tab == tab;
        div()
            .id(SharedString::from(format!("tab-{tab:?}")))
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_md()
            .cursor_pointer()
            .bg(if active {
                theme.sidebar_accent
            } else {
                transparent_black()
            })
            .hover(|s| if active { s } else { s.bg(theme.muted) })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.active_tab = tab;
                    cx.notify();
                }),
            )
            .child(div().size(rems(1.0)).child(icon).text_color(if active {
                theme.sidebar_foreground
            } else {
                theme.muted_foreground
            }))
            .child(
                div()
                    .text_sm()
                    .text_color(if active {
                        theme.sidebar_foreground
                    } else {
                        theme.sidebar_foreground.opacity(0.8)
                    })
                    .child(label.to_string()),
            )
    }

    fn render_general_tab(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        v_flex()
            .gap_8()
            .child(
                v_flex()
                    .gap_6()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::GeneralOptions, lang)),
                    )
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::Language, lang)),
                                    )
                                    .child(div().w(rems(12.5)).child(setting_select(
                                        Select::new(&self.language_select),
                                        theme,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::ThemeStyle, lang)),
                                    )
                                    .child(div().w(rems(12.5)).child(setting_select(
                                        Select::new(&self.theme_style_select),
                                        theme,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::UiScale, lang)),
                                    )
                                    .child(div().w(rems(12.5)).child(setting_select(
                                        Select::new(&self.ui_scale_select),
                                        theme,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::LogLevel, lang)),
                                    )
                                    .child(div().w(rems(12.5)).child(setting_select(
                                        Select::new(&self.log_level_select),
                                        theme,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::NotificationLevel, lang)),
                                    )
                                    .child(div().w(rems(12.5)).child(setting_select(
                                        Select::new(&self.notification_level_select),
                                        theme,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        v_flex()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(t(I18nKey::Appearance, lang)),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground)
                                                    .child(t(I18nKey::ThemeDesc, lang)),
                                            ),
                                    )
                                    .child(self.render_theme_switcher(theme, cx)),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_6()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::LibrarySettings, lang)),
                    )
                    .child(self.render_path_item(
                        t(I18nKey::DatabaseDir, lang),
                        t(I18nKey::DatabaseDirDesc, lang),
                        &self.base_dir_input,
                        theme,
                        cx,
                    ))
                    .child(self.render_path_item(
                        t(I18nKey::AttachmentDir, lang),
                        t(I18nKey::AttachmentDirDesc, lang),
                        &self.attachment_path_input,
                        theme,
                        cx,
                    ))
                    .child(self.render_filename_template_section(lang, theme, cx)),
            )
            .child(self.render_pdf_viewer_section(lang, theme, cx))
            .child(
                v_flex()
                    .gap_6()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::MetadataServices, lang)),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child(t(I18nKey::EasyScholarKey, lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::EasyScholarDesc, lang)),
                            )
                            .child(setting_input(
                                Input::new(&self.easyscholar_key_input),
                                theme,
                            )),
                    ),
            )
            .child(
                v_flex()
                    .gap_6()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::NetworkProxySettings, lang)),
                    )
                    .child(self.render_proxy_section(lang, theme, cx)),
            )
    }

    fn render_proxy_section(
        &self,
        lang: Language,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child(t(I18nKey::EnableProxyServer, lang)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::ProxyDesc, lang)),
                            ),
                    )
                    .child(
                        Switch::new("proxy-enable")
                            .checked(self.proxy_enabled)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.proxy_enabled = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.proxy_enabled, |this| {
                this.child(self.render_input_field(
                    t(I18nKey::ProxyAddress, lang),
                    &self.proxy_url_input,
                    theme,
                    false,
                ))
            })
    }

    fn render_pdf_viewer_section(
        &self,
        lang: Language,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let macos_input_clone = self.pdf_macos_app_input.clone();
        let windows_input_clone = self.pdf_windows_app_input.clone();

        v_flex()
            .gap_6()
            .child(
                v_flex()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::PdfViewerSettings, lang)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(t(I18nKey::PdfViewerSettingsDesc, lang)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(t(I18nKey::InternalReaderDesc, lang)),
                    ),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::UseCustomPdfViewer, lang)),
                    )
                    .child(
                        Switch::new("pdf-use-custom")
                            .checked(self.pdf_use_custom)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.pdf_use_custom = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.pdf_use_custom, |this| {
                this.child(
                    v_flex()
                        .gap_4()
                        // macOS 设置
                        .when(cfg!(target_os = "macos"), |this| {
                            this.child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::PdfViewerPathMacos, lang)),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(setting_input(Input::new(&self.pdf_macos_app_input), theme).flex_grow())
                                            .child(
                                                Button::new("browse-macos-pdf")
                                                    .child(Icon::new(IconName::FolderSelect).size(rems(0.875)))
                                                    .on_click(cx.listener(move |_, _, window, cx| {
                                                        let input_state = macos_input_clone.clone();
                                                        let window_handle = window.window_handle();
                                                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                                                            files: true,
                                                            directories: true,
                                                            multiple: false,
                                                            prompt: Some(t(I18nKey::SelectMacosPdfReader, lang).into()),
                                                        });
                                                        cx.spawn(move |_, cx: &mut AsyncApp| {
                                                            let mut cx = cx.clone();
                                                            async move {
                                                                if let Ok(Ok(Some(paths))) = receiver.await
                                                                    && let Some(path) = paths.first()
                                                                {
                                                                    let path_str = path.to_string_lossy().to_string();
                                                                    let _ = cx.update_window(window_handle, |_, window, cx| {
                                                                        input_state.update(cx, |state, cx| {
                                                                            let len = state.text().len();
                                                                            state.replace_text_in_range(
                                                                                Some(0..len),
                                                                                &path_str,
                                                                                window,
                                                                                cx,
                                                                            );
                                                                        });
                                                                    });
                                                                }
                                                            }
                                                        })
                                                        .detach();
                                                    })),
                                            ),
                                    ),
                            )
                        })
                        // Windows 设置
                        .when(cfg!(target_os = "windows"), |this| {
                            this.child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .child(t(I18nKey::PdfViewerPathWindows, lang)),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(setting_input(Input::new(&self.pdf_windows_app_input), theme).flex_grow())
                                            .child(
                                                Button::new("browse-windows-pdf")
                                                    .child(Icon::new(IconName::FolderSelect).size(rems(0.875)))
                                                    .on_click(cx.listener(move |_, _, window, cx| {
                                                        let input_state = windows_input_clone.clone();
                                                        let window_handle = window.window_handle();
                                                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                                                            files: true,
                                                            directories: false,
                                                            multiple: false,
                                                            prompt: Some(t(I18nKey::SelectWindowsPdfReader, lang).into()),
                                                        });
                                                        cx.spawn(move |_, cx: &mut AsyncApp| {
                                                            let mut cx = cx.clone();
                                                            async move {
                                                                if let Ok(Ok(Some(paths))) = receiver.await
                                                                    && let Some(path) = paths.first()
                                                                {
                                                                    let path_str = path.to_string_lossy().to_string();
                                                                    let _ = cx.update_window(window_handle, |_, window, cx| {
                                                                        input_state.update(cx, |state, cx| {
                                                                            let len = state.text().len();
                                                                            state.replace_text_in_range(
                                                                                Some(0..len),
                                                                                &path_str,
                                                                                window,
                                                                                cx,
                                                                            );
                                                                        });
                                                                    });
                                                                }
                                                            }
                                                        })
                                                        .detach();
                                                    })),
                                            ),
                                    ),
                            )
                        }),
                )
            })
    }

    fn render_theme_switcher(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        h_flex()
            .bg(theme.muted)
            .rounded_md()
            .p_1()
            .child(self.render_theme_item("light", t(I18nKey::Light, lang), theme, cx))
            .child(self.render_theme_item("dark", t(I18nKey::Dark, lang), theme, cx))
            .child(self.render_theme_item("system", t(I18nKey::System, lang), theme, cx))
    }

    fn render_theme_item(
        &self,
        value: &'static str,
        label: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.config.ui.theme_mode == value;
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .bg(if active {
                theme.foreground.opacity(0.5)
            } else {
                transparent_black()
            })
            .text_color(if active {
                gpui::white()
            } else {
                theme.foreground
            })
            .text_sm()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.set_theme_mode(value, cx);
                }),
            )
            .child(label.to_string())
    }

    fn render_filename_template_section(
        &self,
        lang: Language,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let template = self.filename_template_input.read(cx).text().to_string();
        let preview_options = filename::FilenameOptions::new(
            "He",
            "Kaiming",
            "2022",
            "Masked Autoencoders Are Scalable Vision Learners",
            "CVPR",
            "pdf",
            true,
        );
        let preview_name = filename::generate_filename_from_template(&template, &preview_options);

        v_flex()
            .gap_2()
            .w_full()
            .child(
                v_flex()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::FilenameTemplate, lang)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .whitespace_normal()
                            .child(t(I18nKey::FilenameTemplateDesc, lang)),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        setting_input(Input::new(&self.filename_template_input), theme).flex_grow(),
                    )
                    .child(
                        Button::new("batch-rename")
                            .child(t(I18nKey::BatchRename, lang))
                            .small()
                            .w(rems(4.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let app = this.app.clone();
                                cx.background_executor()
                                    .spawn(async move {
                                        if let Err(e) = app.batch_rename_files() {
                                            error!("批量重命名失败: {e}");
                                        }
                                    })
                                    .detach();
                            })),
                    )
                    .child(
                        Button::new("cleanup-orphaned")
                            .child(t(I18nKey::CleanupOrphanedFiles, lang))
                            .small()
                            .w(rems(4.5))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let app = this.app.clone();
                                cx.background_executor()
                                    .spawn(async move {
                                        if let Err(e) = app.cleanup_orphaned_files() {
                                            error!("清理孤立文件失败: {e}");
                                        }
                                    })
                                    .detach();
                            })),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}: ", t(I18nKey::Preview, lang))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.primary)
                            .child(preview_name),
                    ),
            )
    }

    fn render_input_field(
        &self,
        label: &str,
        input: &Entity<InputState>,
        theme: &Theme,
        disabled: bool,
    ) -> impl IntoElement {
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(if disabled {
                        theme.muted_foreground.opacity(0.5)
                    } else {
                        theme.muted_foreground
                    })
                    .child(label.to_string()),
            )
            .child(
                div()
                    .when(disabled, |s: gpui::Div| s.opacity(0.5).cursor_not_allowed())
                    .child(setting_input(Input::new(input), theme)),
            )
    }

    fn render_password_field(
        &self,
        label: &str,
        input: &Entity<InputState>,
        theme: &Theme,
        disabled: bool,
    ) -> impl IntoElement {
        v_flex()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(if disabled {
                        theme.muted_foreground.opacity(0.5)
                    } else {
                        theme.muted_foreground
                    })
                    .child(label.to_string()),
            )
            .child(
                div()
                    .when(disabled, |s: gpui::Div| s.opacity(0.5).cursor_not_allowed())
                    .child(setting_input(Input::new(input).mask_toggle(), theme)),
            )
    }

    fn render_sync_section<F, E>(
        &self,
        title: &str,
        enabled: bool,
        switch_id: &'static str,
        theme: &Theme,
        cx: &mut Context<Self>,
        content_builder: F,
    ) -> impl IntoElement
    where
        F: FnOnce(&Theme, &mut Context<Self>) -> E,
        E: IntoElement,
    {
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .child(title.to_string()),
                    )
                    .child(
                        Switch::new(switch_id)
                            .checked(enabled)
                            .on_click(cx.listener(move |this, checked, _, cx| {
                                match switch_id {
                                    "webdav-enable" => this.webdav_enabled = *checked,
                                    "db-remote-enable" => this.db_use_remote = *checked,
                                    _ => {}
                                }
                                cx.notify();
                            })),
                    ),
            )
            .child(div().when(enabled, |this| {
                this.p_4()
                    .bg(theme.muted.opacity(0.2))
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .child(content_builder(theme, cx))
            }))
    }

    fn render_sync_tab(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        v_flex()
            .gap_6()
            .w_full()
            .child(
                h_flex()
                    .justify_between()
                    .items_end()
                    .w_full()
                    .pb_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(t(I18nKey::Sync, lang)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .max_w(rems(25.0))
                                    .whitespace_normal()
                                    .child(t(I18nKey::CloudSyncDesc, lang)),
                            ),
                    ),
            )
            // 子标签导航
            .child(self.render_sync_sub_tabs(theme, cx))
            // 子标签内容
            .child(match self.sync_sub_tab {
                SyncSubTab::Metadata => self.render_metadata_sync(theme, cx).into_any_element(),
                SyncSubTab::Attachment => self.render_attachment_sync(theme, cx).into_any_element(),
            })
            // 数据管理
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::DataManagement, lang)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new("clear-local-db")
                                    .child(t(I18nKey::ClearLocalDb, lang))
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let app = this.app.clone();
                                        cx.background_executor()
                                            .spawn(async move {
                                                if let Err(e) = app.clear_local_database() {
                                                    error!("清空本地数据库失败: {e}");
                                                }
                                            })
                                            .detach();
                                    })),
                            )
                            .child(
                                Button::new("clear-local-files")
                                    .child(t(I18nKey::ClearLocalFiles, lang))
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let app = this.app.clone();
                                        cx.background_executor()
                                            .spawn(async move {
                                                if let Err(e) = app.file_manager.trash_all() {
                                                    error!("清空本地文件失败: {e}");
                                                }
                                            })
                                            .detach();
                                    })),
                            )
                            .child(
                                Button::new("clear-cloud-db")
                                    .child(t(I18nKey::ClearCloudDb, lang))
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let app = this.app.clone();
                                        cx.background_executor()
                                            .spawn(async move {
                                                if let Err(e) =
                                                    app.sync_service.clear_remote_database().await
                                                {
                                                    error!("清空云数据库失败: {e}");
                                                }
                                            })
                                            .detach();
                                    })),
                            )
                            .child(
                                Button::new("clear-cloud-files")
                                    .child(t(I18nKey::ClearCloudFiles, lang))
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let app = this.app.clone();
                                        cx.background_executor()
                                            .spawn(async move {
                                                if let Err(e) =
                                                    app.sync_service.clear_remote_files().await
                                                {
                                                    error!("清空云端文件失败: {e}");
                                                }
                                            })
                                            .detach();
                                    })),
                            ),
                    ),
            )
    }

    fn render_sync_sub_tabs(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        h_flex()
            .gap_1()
            .bg(theme.muted)
            .rounded_md()
            .p_1()
            .child(self.render_sub_tab_item(
                SyncSubTab::Metadata,
                t(I18nKey::SyncMetadataTab, lang),
                theme,
                cx,
            ))
            .child(self.render_sub_tab_item(
                SyncSubTab::Attachment,
                t(I18nKey::SyncAttachmentTab, lang),
                theme,
                cx,
            ))
    }

    fn render_sub_tab_item(
        &self,
        tab: SyncSubTab,
        label: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.sync_sub_tab == tab;
        div()
            .id(SharedString::from(format!("sync-sub-{tab:?}")))
            .px_4()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .bg(if active {
                theme.background
            } else {
                transparent_black()
            })
            .text_color(if active {
                theme.foreground
            } else {
                theme.muted_foreground
            })
            .text_sm()
            .font_weight(if active {
                FontWeight::BOLD
            } else {
                FontWeight::default()
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.sync_sub_tab = tab;
                    cx.notify();
                }),
            )
            .child(label.to_string())
    }

    fn render_metadata_sync(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        // 数据库设置
        self.render_sync_section(t(I18nKey::DatabaseSettings, lang), self.db_use_remote, "db-remote-enable", theme, cx, move |theme, cx| {
            v_flex().gap_4()
                .child(
                    h_flex().gap_4()
                        .child(div().flex_grow().child(self.render_input_field(t(I18nKey::Host, lang), &self.db_host_input, theme, false)))
                        .child(div().w(rems(5.0)).child(self.render_input_field(t(I18nKey::Port, lang), &self.db_port_input, theme, false)))
                )
                .child(self.render_input_field(t(I18nKey::DatabaseName, lang), &self.db_name_input, theme, false))
                .child(
                    h_flex().gap_4()
                        .child(div().flex_1().child(self.render_input_field(t(I18nKey::Username, lang), &self.db_user_input, theme, false)))
                        .child(div().flex_1().child(self.render_input_field(t(I18nKey::Password, lang), &self.db_pass_input, theme, false)))
                )
                .child(
                    h_flex().justify_between().pt_2()
                        .child(
                            h_flex().gap_2()
                                .child(Switch::new("db-ssl-enable").checked(self.db_use_ssl).on_click(cx.listener(|this, checked, _, cx| {
                                    this.db_use_ssl = *checked;
                                    cx.notify();
                                })))
                                .child(div().text_sm().child(t(I18nKey::EnableSSL, lang)))
                        )
                        .child(
                            h_flex().gap_2()
                                .child(
                                    h_flex().gap_1()
                                        .when_some(self.db_test_result.as_ref(), |this, res| {
                                            match res {
                                                Ok(()) => this.child(Icon::new(IconName::Check).size(rems(0.875)).text_color(gpui::green()))
                                                            .child(div().text_xs().text_color(gpui::green()).child(t(I18nKey::ConnectionSuccess, lang))),
                                                Err(_) => this.child(Icon::new(IconName::TriangleAlert).size(rems(0.875)).text_color(gpui::red()))
                                                            .child(div().text_xs().text_color(gpui::red()).child(t(I18nKey::ConnectionFailed, lang))),
                                            }
                                        })
                                )
                                .when(self.db_tested, |s: gpui::Div| {
                                    s.child(
                                        Button::new("sync-metadata-inline")
                                            .child(h_flex().gap_2().child(Icon::new(IconName::Globe).size(rems(0.875))).child(t(I18nKey::SyncMetadata, lang)))
                                            .small()
                                            .primary()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let app = this.app.clone();
                                                cx.background_executor().spawn(async move {
                                                    app.perform_sync();
                                                }).detach();
                                            }))
                                    )
                                })
                                .child(Button::new("test-db")
                                    .child(t(I18nKey::TestConnection, lang))
                                    .small()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let app = this.app.clone();
                                        let window_handle = window.window_handle();

                                        let mut test_config = this.config.database.clone();
                                        test_config.use_remote = true;
                                        test_config.host = this.db_host_input.read(cx).text().to_string();
                                        test_config.port = this.db_port_input.read(cx).text().to_string().parse().unwrap_or(3306);
                                        test_config.database = this.db_name_input.read(cx).text().to_string();
                                        test_config.username = this.db_user_input.read(cx).text().to_string();
                                        test_config.password = this.db_pass_input.read(cx).text().to_string();
                                        test_config.use_ssl = this.db_use_ssl;

                                        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                            let mut cx = cx.clone();
                                            async move {
                                                let _guard = RUNTIME.enter();
                                                let mysql_res = app.test_mysql_config(test_config).await;
                                                let is_ok = mysql_res.is_ok();

                                                let _ = cx.update_window(window_handle, |_, _, cx| {
                                                    if let Some(this) = this.upgrade() {
                                                        this.update(cx, |this: &mut Self, cx: &mut Context<Self>| {
                                                            this.db_tested = is_ok;
                                                            if let Err(ref e) = mysql_res {
                                                                let err_msg = e.clone();
                                                                let lang = this.config.ui.language.parse::<Language>().unwrap_or_default();
                                                                show_notification(NotificationType::Error, format!("{}: {}", t(I18nKey::ConnectionFailed, lang), err_msg), cx);
                                                            }
                                                            this.db_test_result = Some(mysql_res);
                                                            cx.notify();
                                                        });
                                                    }
                                                });
                                            }
                                        }).detach();
                                    })))
                        )
                )
        })
    }

    fn render_attachment_sync(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        v_flex().gap_6()
            // WebDAV 设置
            .child(
                self.render_sync_section(t(I18nKey::WebDavSettings, lang), self.webdav_enabled, "webdav-enable", theme, cx, |theme, cx| {
                    v_flex().gap_4()
                        .child(self.render_input_field(t(I18nKey::EndpointUrl, lang), &self.webdav_endpoint_input, theme, false))
                        .child(
                            h_flex().gap_4()
                                .child(div().flex_1().child(self.render_input_field(t(I18nKey::Username, lang), &self.webdav_username_input, theme, false)))
                                .child(div().flex_1().child(self.render_input_field(t(I18nKey::Password, lang), &self.webdav_password_input, theme, false)))
                        )
                        .child(self.render_input_field(t(I18nKey::RemotePath, lang), &self.webdav_remote_path_input, theme, false))
                        .child(
                            h_flex().justify_between().pt_2()
                                .child(
                                    h_flex().gap_2()
                                        .child(Switch::new("webdav-on-demand").checked(self.webdav_on_demand).on_click(cx.listener(|this, checked, _, cx| {
                                            this.webdav_on_demand = *checked;
                                            cx.notify();
                                        })))
                                        .child(
                                            v_flex().gap_0()
                                                .child(div().text_sm().child(t(I18nKey::OnDemandDownload, lang)))
                                                .child(div().text_xs().text_color(theme.muted_foreground).child(t(I18nKey::OnDemandDownloadDesc, lang)))
                                        )
                                )
                        )
                        .child(
                            h_flex().justify_end().gap_4().pt_2()
                                .child(
                                    h_flex().gap_1()
                                        .when_some(self.webdav_test_result.as_ref(), |this, res| {
                                            match res {
                                                Ok(()) => this.child(Icon::new(IconName::Check).size(rems(0.875)).text_color(gpui::green()))
                                                            .child(div().text_xs().text_color(gpui::green()).child(t(I18nKey::ConnectionSuccess, lang))),
                                                Err(_) => this.child(Icon::new(IconName::TriangleAlert).size(rems(0.875)).text_color(gpui::red()))
                                                            .child(div().text_xs().text_color(gpui::red()).child(t(I18nKey::ConnectionFailed, lang))),
                                            }
                                        })
                                )
                                .when(self.webdav_tested, |s: gpui::Div| {
                                    s.child(
                                        Button::new("sync-attachments-inline")
                                            .child(h_flex().gap_2().child(Icon::new(IconName::File).size(rems(0.875))).child(t(I18nKey::SyncAttachments, lang)))
                                            .small()
                                            .primary()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let app = this.app.clone();
                                                RUNTIME.spawn(async move {
                                                    app.perform_attachments_sync();
                                                });
                                                cx.notify();
                                            }))
                                    )
                                })
                                .child(Button::new("test-webdav")
                                    .child(t(I18nKey::TestConnection, lang))
                                    .small()
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let app = this.app.clone();
                                        let window_handle = window.window_handle();

                                        let endpoint = this.webdav_endpoint_input.read(cx).text().to_string();
                                        let username = this.webdav_username_input.read(cx).text().to_string();
                                        let password = this.webdav_password_input.read(cx).text().to_string();
                                        let remote_path = this.webdav_remote_path_input.read(cx).text().to_string();

                                        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                            let mut cx = cx.clone();
                                            async move {
                                                let _guard = RUNTIME.enter();
                                                let webdav_res = app.test_webdav_config(endpoint, username, password, remote_path).await;
                                                let is_ok = webdav_res.is_ok();

                                                let _ = cx.update_window(window_handle, |_, _, cx| {
                                                    if let Some(this) = this.upgrade() {
                                                        this.update(cx, |this: &mut Self, cx: &mut Context<Self>| {
                                                            this.webdav_tested = is_ok;
                                                            if let Err(ref e) = webdav_res {
                                                                let err_msg = e.clone();
                                                                let lang = this.config.ui.language.parse::<Language>().unwrap_or_default();
                                                                show_notification(NotificationType::Error, format!("{}: {}", t(I18nKey::ConnectionFailed, lang), err_msg), cx);
                                                            }
                                                            this.webdav_test_result = Some(webdav_res);
                                                            cx.notify();
                                                        });
                                                    }
                                                });
                                            }
                                        }).detach();
                                    })))
                        )
                })
            )
            // Google Drive 设置
            .child(self.render_google_drive_section(theme, cx))
    }

    fn render_google_drive_section(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::GoogleDriveSettings, lang)),
                    )
                    .child(
                        Switch::new("google-drive-enable")
                            .checked(self.google_drive_enabled)
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.google_drive_enabled = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().when(self.google_drive_enabled, |this| {
                this.p_4()
                    .bg(theme.muted.opacity(0.2))
                    .rounded_lg()
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::GoogleDriveDesc, lang)),
                            )
                            .child(self.render_input_field(
                                t(I18nKey::ClientId, lang),
                                &self.google_drive_client_id_input,
                                theme,
                                false,
                            ))
                            .child(self.render_input_field(
                                t(I18nKey::ClientSecret, lang),
                                &self.google_drive_client_secret_input,
                                theme,
                                false,
                            ))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Icon::new(if self.google_drive_authorized {
                                                    IconName::Check
                                                } else {
                                                    IconName::TriangleAlert
                                                })
                                                .size(rems(0.875))
                                                .text_color(if self.google_drive_authorized {
                                                    gpui::green()
                                                } else {
                                                    gpui::red()
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(if self.google_drive_authorized {
                                                        gpui::green()
                                                    } else {
                                                        theme.muted_foreground
                                                    })
                                                    .child(if self.google_drive_authorized {
                                                        t(I18nKey::ConnectionSuccess, lang)
                                                    } else {
                                                        t(I18nKey::ConnectionFailed, lang)
                                                    }),
                                            ),
                                    )
                                    .child(
                                        Button::new("authorize-google-drive")
                                            .child(t(I18nKey::Authorize, lang))
                                            .primary()
                                            .small()
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                let window_handle = window.window_handle();
                                                let client_id = this.google_drive_client_id_input.read(cx).text().to_string();
                                                let client_secret = this.google_drive_client_secret_input.read(cx).text().to_string();

                                                cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
                                                    let mut cx = cx.clone();
                                                    async move {
                                                        let _guard = RUNTIME.enter();
                                                        let result = sync::google_drive::complete_oauth_flow(&client_id, &client_secret).await;

                                                        let err_msg = result.as_ref().err().map(|e| e.to_string());
                                                        let is_ok = result.is_ok();

                                                        let window_result = cx.update_window(window_handle, |_, _, cx| {
                                                            if let Some(this) = this.upgrade() {
                                                                let lang = this.update(cx, |this, _| {
                                                                    if let Ok(refresh_token) = result {
                                                                        this.google_drive_authorized = true;
                                                                        if let Ok(mut state) = this.app.local_state.write() {
                                                                            state.google_drive_refresh_token = refresh_token;
                                                                            let _ = this.app.local_state_manager.save_all(&state);
                                                                            debug!("OAuth 回调: 已保存 refresh_token");
                                                                        } else {
                                                                            error!("OAuth 回调: local_state.write() 失败");
                                                                        }
                                                                    }
                                                                    this.config.ui.language.parse::<Language>().unwrap_or_default()
                                                                });
                                                                if is_ok {
                                                                    show_notification(NotificationType::Success, t(I18nKey::ConnectionSuccess, lang), cx);
                                                                } else if let Some(ref e) = err_msg {
                                                                    error!("OAuth 回调: 授权失败: {e}");
                                                                    show_notification(NotificationType::Error, format!("{}: {e}", t(I18nKey::ConnectionFailed, lang)), cx);
                                                                }
                                                                cx.refresh_windows();
                                                            } else {
                                                                error!("OAuth 回调: SettingsWindow 已被释放");
                                                            }
                                                        });
                                                        if let Err(e) = window_result {
                                                            error!("OAuth 回调: update_window 失败: {e:?}");
                                                        }
                                                    }
                                                }).detach();
                                            })),
                                    ),
                            )

                            .child(h_flex().justify_end().gap_2().when(
                                self.google_drive_authorized,
                                |s| {
                                    s.child(
                                        Button::new("sync-google-drive")
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        Icon::new(IconName::File).size(rems(0.875)),
                                                    )
                                                    .child(t(I18nKey::SyncAttachments, lang)),
                                            )
                                            .small()
                                            .primary()
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let app = this.app.clone();
                                                RUNTIME.spawn(async move {
                                                    app.perform_attachments_sync();
                                                });
                                                cx.notify();
                                            })),
                                    )
                                },
                            )),
                    )
            }))
    }

    fn render_ai_backends_tab(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        let editing = self.ai_edit_target.is_some() || self.ai_adding_new;

        let mut cards: Vec<gpui::AnyElement> = Vec::new();

        if self.ai_entries.is_empty() {
            cards.push(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t(I18nKey::AiNoBackends, lang))
                    .into_any_element(),
            );
        }

        for (i, entry) in self.ai_entries.iter().enumerate() {
            let kind_label = match entry.kind.to_lowercase().as_str() {
                "ollama" => "Ollama",
                "claude" => "Claude",
                "siliconflow" => "SiliconFlow",
                _ => "OpenAI",
            };
            let api_base = entry.api_base.clone();
            let model = entry.model.clone();

            let card = v_flex()
                .gap_2()
                .p_3()
                .bg(theme.muted.opacity(0.15))
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child(entry.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .px_1()
                                        .rounded_sm()
                                        .bg(theme.muted)
                                        .text_color(theme.muted_foreground)
                                        .child(kind_label),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new(SharedString::from(format!("ai-edit-{i}")))
                                        .child(t(I18nKey::Edit, lang))
                                        .small()
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            if let Some(entry) = this.ai_entries.get(i) {
                                                let name_state = this.ai_edit_name_input.clone();
                                                let key_state = this.ai_edit_api_key_input.clone();
                                                let base_state =
                                                    this.ai_edit_api_base_input.clone();
                                                let model_state = this.ai_edit_model_input.clone();
                                                name_state.update(cx, |s, cx| {
                                                    let l = s.text().len();
                                                    s.replace_text_in_range(
                                                        Some(0..l),
                                                        &entry.name,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                                this.ai_edit_kind_value = entry.kind.clone();
                                                this.ai_edit_kind_select.update(cx, |s, cx| {
                                                    s.set_selected_value(&entry.kind, window, cx);
                                                });
                                                key_state.update(cx, |s, cx| {
                                                    let l = s.text().len();
                                                    s.replace_text_in_range(
                                                        Some(0..l),
                                                        &entry.api_key,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                                base_state.update(cx, |s, cx| {
                                                    let l = s.text().len();
                                                    s.replace_text_in_range(
                                                        Some(0..l),
                                                        &entry.api_base,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                                model_state.update(cx, |s, cx| {
                                                    let l = s.text().len();
                                                    s.replace_text_in_range(
                                                        Some(0..l),
                                                        &entry.model,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                                let ctx_state =
                                                    this.ai_edit_context_window_input.clone();
                                                ctx_state.update(cx, |s, cx| {
                                                    let l = s.text().len();
                                                    s.replace_text_in_range(
                                                        Some(0..l),
                                                        &entry.context_window.to_string(),
                                                        window,
                                                        cx,
                                                    );
                                                });
                                                this.ai_edit_compression_strategy_value =
                                                    entry.compression_strategy.clone();
                                                this.ai_edit_compression_strategy_select.update(
                                                    cx,
                                                    |s, cx| {
                                                        s.set_selected_value(
                                                            &entry.compression_strategy,
                                                            window,
                                                            cx,
                                                        );
                                                    },
                                                );
                                                this.ai_edit_target = Some(i);
                                                this.ai_edit_enable_thinking =
                                                    entry.enable_thinking;
                                                this.ai_adding_new = false;
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(SharedString::from(format!("ai-delete-{i}")))
                                        .child(t(I18nKey::Delete, lang))
                                        .small()
                                        .on_click(cx.listener({
                                            let name = entry.name.clone();
                                            move |this, _, _, cx| {
                                                if i < this.ai_entries.len() {
                                                    this.ai_entries.remove(i);
                                                    if this.ai_active_name == name {
                                                        this.ai_active_name = this
                                                            .ai_entries
                                                            .first()
                                                            .map(|e| e.name.clone())
                                                            .unwrap_or_default();
                                                    }
                                                    cx.notify();
                                                }
                                            }
                                        })),
                                ),
                        ),
                )
                .child(
                    h_flex()
                        .gap_4()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("API: {api_base}"))
                        .child(format!("Model: {model}")),
                );
            cards.push(card.into_any_element());

            if self.ai_edit_target == Some(i) {
                cards.push(self.render_ai_edit_form(theme, cx));
            }
        }

        if self.ai_adding_new {
            cards.push(self.render_ai_edit_form(theme, cx));
        }

        if !editing {
            cards.push(
                Button::new("ai-add-backend")
                    .child(t(I18nKey::AiAddBackend, lang))
                    .small()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let name_state = this.ai_edit_name_input.clone();
                        let key_state = this.ai_edit_api_key_input.clone();
                        let base_state = this.ai_edit_api_base_input.clone();
                        let model_state = this.ai_edit_model_input.clone();
                        name_state.update(cx, |s, cx| {
                            let l = s.text().len();
                            s.replace_text_in_range(Some(0..l), "", window, cx);
                        });
                        this.ai_edit_kind_value = "openai".to_string();
                        this.ai_edit_kind_select.update(cx, |s, cx| {
                            s.set_selected_value(&"openai".to_string(), window, cx);
                        });
                        key_state.update(cx, |s, cx| {
                            let l = s.text().len();
                            s.replace_text_in_range(Some(0..l), "", window, cx);
                        });
                        base_state.update(cx, |s, cx| {
                            let l = s.text().len();
                            s.replace_text_in_range(
                                Some(0..l),
                                "https://api.openai.com/v1",
                                window,
                                cx,
                            );
                        });
                        model_state.update(cx, |s, cx| {
                            let l = s.text().len();
                            s.replace_text_in_range(Some(0..l), "gpt-4o-mini", window, cx);
                        });
                        let ctx_state = this.ai_edit_context_window_input.clone();
                        ctx_state.update(cx, |s, cx| {
                            let l = s.text().len();
                            s.replace_text_in_range(Some(0..l), "128000", window, cx);
                        });
                        this.ai_edit_compression_strategy_value = "sliding_window".to_string();
                        this.ai_edit_compression_strategy_select
                            .update(cx, |s, cx| {
                                s.set_selected_value(&"sliding_window".to_string(), window, cx);
                            });
                        this.ai_adding_new = true;
                        this.ai_edit_target = None;
                        this.ai_edit_enable_thinking = false;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        v_flex().gap_3().children(cards)
    }

    fn render_ai_edit_form(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        v_flex()
            .gap_3()
            .p_4()
            .bg(theme.muted.opacity(0.2))
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .child(if self.ai_adding_new {
                        t(I18nKey::AiAddBackend, lang)
                    } else {
                        t(I18nKey::Edit, lang)
                    }),
            )
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::AiBackendName, lang)),
                            )
                            .child(setting_input(Input::new(&self.ai_edit_name_input), theme)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::AiBackendType, lang)),
                            )
                            .child(setting_select(
                                Select::new(&self.ai_edit_kind_select),
                                theme,
                            )),
                    ),
            )
            .child(self.render_password_field(
                t(I18nKey::AiApiKey, lang),
                &self.ai_edit_api_key_input,
                theme,
                false,
            ))
            .when(
                self.ai_edit_kind_value.to_lowercase().as_str() != "siliconflow",
                |parent| {
                    parent.child(self.render_input_field(
                        t(I18nKey::AiApiBase, lang),
                        &self.ai_edit_api_base_input,
                        theme,
                        false,
                    ))
                },
            )
            .child(
                h_flex()
                    .gap_4()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::AiModel, lang)),
                            )
                            .child(setting_input(Input::new(&self.ai_edit_model_input), theme)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::AiContextWindow, lang)),
                            )
                            .child(setting_input(
                                Input::new(&self.ai_edit_context_window_input),
                                theme,
                            )),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::AiCompressionStrategy, lang)),
                            )
                            .child(setting_select(
                                Select::new(&self.ai_edit_compression_strategy_select),
                                theme,
                            )),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Switch::new("ai-edit-enable-thinking")
                            .checked(self.ai_edit_enable_thinking)
                            .on_click(cx.listener(|this, checked, _, cx| {
                                this.ai_edit_enable_thinking = *checked;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("启用思考过程 (Enable Thinking)"),
                    ),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("ai-edit-cancel")
                            .child(t(I18nKey::Cancel, lang))
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.ai_edit_target = None;
                                this.ai_adding_new = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("ai-edit-save")
                            .child(t(I18nKey::Save, lang))
                            .small()
                            .primary()
                            .on_click(cx.listener(|this, _, _window, cx| {
                                let name = this.ai_edit_name_input.read(cx).text().to_string();
                                let kind = this.ai_edit_kind_value.clone();
                                let api_key =
                                    this.ai_edit_api_key_input.read(cx).text().to_string();
                                let api_base =
                                    this.ai_edit_api_base_input.read(cx).text().to_string();
                                let model = this.ai_edit_model_input.read(cx).text().to_string();
                                let context_window_str = this
                                    .ai_edit_context_window_input
                                    .read(cx)
                                    .text()
                                    .to_string();
                                let context_window: u32 =
                                    context_window_str.parse().unwrap_or(128000);
                                let compression_strategy =
                                    this.ai_edit_compression_strategy_value.clone();

                                let effective_kind = if kind.is_empty() {
                                    "openai".to_string()
                                } else {
                                    kind.clone()
                                };
                                let default_base = match effective_kind.to_lowercase().as_str() {
                                    "claude" => "https://api.anthropic.com",
                                    "ollama" => "http://localhost:11434",
                                    "siliconflow" => "https://api.siliconflow.cn",
                                    _ => "https://api.openai.com/v1",
                                };
                                let default_model = match effective_kind.to_lowercase().as_str() {
                                    "claude" => "claude-sonnet-4-20250514",
                                    "ollama" => "qwen2.5",
                                    "siliconflow" => "THUDM/GLM-Z1-9B-0414",
                                    _ => "gpt-4o-mini",
                                };

                                let new_entry = translate::AiBackendEntry {
                                    name: if name.is_empty() {
                                        "unnamed".into()
                                    } else {
                                        name
                                    },
                                    kind: effective_kind.clone(),
                                    api_key,
                                    api_base: if effective_kind.to_lowercase().as_str()
                                        == "siliconflow"
                                    {
                                        "https://api.siliconflow.cn".into()
                                    } else if api_base.is_empty() {
                                        default_base.into()
                                    } else {
                                        api_base
                                    },
                                    model: if model.is_empty() {
                                        default_model.into()
                                    } else {
                                        model
                                    },
                                    temperature: 0.3,
                                    max_tokens: 4096,
                                    context_window,
                                    compression_strategy,
                                    enable_thinking: this.ai_edit_enable_thinking,
                                };

                                if this.ai_adding_new {
                                    this.ai_entries.push(new_entry);
                                    if this.ai_active_name.is_empty() {
                                        this.ai_active_name = this
                                            .ai_entries
                                            .last()
                                            .map(|e| e.name.clone())
                                            .unwrap_or_default();
                                    }
                                } else if let Some(idx) = this.ai_edit_target
                                    && idx < this.ai_entries.len()
                                {
                                    this.ai_entries[idx] = new_entry;
                                }

                                this.ai_edit_target = None;
                                this.ai_adding_new = false;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_translation_ai_selector(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        if self.ai_entries.is_empty() {
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(t(I18nKey::AiNoBackends, lang))
                .into_any_element()
        } else {
            h_flex()
                .gap_2()
                .flex_wrap()
                .children(self.ai_entries.iter().map(|entry| {
                    let is_active = entry.name == self.ai_active_name;
                    let name = entry.name.clone();
                    div()
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .bg(if is_active {
                            theme.primary
                        } else {
                            theme.muted
                        })
                        .text_color(if is_active {
                            gpui::white()
                        } else {
                            theme.foreground
                        })
                        .text_sm()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.ai_active_name = name.clone();
                                cx.notify();
                            }),
                        )
                        .child(entry.name.clone())
                }))
                .into_any_element()
        }
    }

    fn render_translation_tab(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        v_flex()
            .gap_10()
            .w_full()
            .child(
                h_flex()
                    .justify_between()
                    .items_end()
                    .w_full()
                    .pb_4()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_2xl()
                                    .font_weight(FontWeight::BOLD)
                                    .child(t(I18nKey::TranslationSettings, lang)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .max_w(rems(25.0))
                                    .whitespace_normal()
                                    .child(t(I18nKey::TranslationSettingsDesc, lang)),
                            ),
                    ),
            )
            .child(
                v_flex().gap_6().child(
                    v_flex()
                        .gap_4()
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child(t(I18nKey::TranslationEngine, lang)),
                                )
                                .child(div().w(rems(12.5)).child(setting_select(
                                    Select::new(&self.translation_engine_select),
                                    theme,
                                ))),
                        )
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child(t(I18nKey::TargetLanguage, lang)),
                                )
                                .child(div().w(rems(12.5)).child(setting_select(
                                    Select::new(&self.target_language_select),
                                    theme,
                                ))),
                        )
                        .child(div().when(
                            translate::ENGINES.iter().any(|e| {
                                e.id == self.config.translation.engine
                                    && e.requires_keys.contains(&"google")
                            }),
                            |this| {
                                this.child(self.render_input_field(
                                    t(I18nKey::GoogleApiKey, lang),
                                    &self.google_api_key_input,
                                    theme,
                                    false,
                                ))
                            },
                        ))
                        .child(div().when(
                            translate::ENGINES.iter().any(|e| {
                                e.id == self.config.translation.engine
                                    && e.requires_keys.contains(&"niutrans")
                            }),
                            |this| {
                                this.child(self.render_input_field(
                                    t(I18nKey::NiuTransApiKey, lang),
                                    &self.niutrans_api_key_input,
                                    theme,
                                    false,
                                ))
                            },
                        ))
                        .child(div().when(
                            translate::ENGINES.iter().any(|e| {
                                e.id == self.config.translation.engine
                                    && e.requires_keys.contains(&"baidu")
                            }),
                            |this| {
                                this.child(self.render_input_field(
                                    t(I18nKey::BaiduApiKey, lang),
                                    &self.baidu_api_key_input,
                                    theme,
                                    false,
                                ))
                            },
                        ))
                        .child(div().when(
                            translate::ENGINES.iter().any(|e| {
                                e.id == self.config.translation.engine
                                    && e.requires_keys.contains(&"youdao")
                            }),
                            |this| {
                                this.child(self.render_input_field(
                                    t(I18nKey::YoudaoApiKey, lang),
                                    &self.youdao_api_key_input,
                                    theme,
                                    false,
                                ))
                            },
                        ))
                        .child(div().when(
                            translate::ENGINES.iter().any(|e| {
                                e.id == self.config.translation.engine
                                    && e.requires_keys.contains(&"deepl")
                            }),
                            |this| {
                                this.child(self.render_input_field(
                                    t(I18nKey::DeepLApiKey, lang),
                                    &self.deepl_api_key_input,
                                    theme,
                                    false,
                                ))
                            },
                        ))
                        .when(self.config.translation.engine == "ai", |this| {
                            this.child(self.render_translation_ai_selector(theme, cx))
                        })
                        .child(
                            div().when(
                                translate::ENGINES
                                    .iter()
                                    .any(|e| e.id == self.config.translation.engine && e.is_free),
                                |_this| {
                                    div()
                                        .p_3()
                                        .bg(theme.muted.opacity(0.3))
                                        .rounded_md()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child(t(I18nKey::NoApiKeyRequired, lang))
                                },
                            ),
                        ),
                ),
            )
    }

    fn render_ai_chat_tab(&self, theme: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        v_flex().gap_6().child(
            v_flex()
                .gap_4()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::BOLD)
                        .child(t(I18nKey::AiChatSettingsTab, lang)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child(t(I18nKey::TranslationEngine, lang)),
                        )
                        .child(if self.ai_entries.is_empty() {
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t(I18nKey::AiNoBackends, lang))
                                .into_any_element()
                        } else {
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .children(self.ai_entries.iter().map(|entry| {
                                    let is_active = entry.name == self.chat_active_name;
                                    let name = entry.name.clone();
                                    div()
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .bg(if is_active {
                                            theme.primary
                                        } else {
                                            theme.muted
                                        })
                                        .text_color(if is_active {
                                            gpui::white()
                                        } else {
                                            theme.foreground
                                        })
                                        .text_sm()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                this.chat_active_name = name.clone();
                                                cx.notify();
                                            }),
                                        )
                                        .child(entry.name.clone())
                                }))
                                .into_any_element()
                        }),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child(t(I18nKey::DefaultSystemPrompt, lang)),
                        )
                        .child(setting_input(
                            Input::new(&self.chat_default_system_prompt_input).h(rems(10.0)),
                            theme,
                        )),
                ),
        )
    }

    fn render_about_tab(&self, theme: &Theme) -> impl IntoElement {
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        v_flex()
            .items_center()
            .justify_center()
            .size_full()
            .gap_6()
            .child(
                div()
                    .size(rems(5.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .child(gpui::img("icons/app_icon.png").size(rems(6.0))),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Lumen"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{} v{}",
                                t(I18nKey::Version, lang),
                                env!("CARGO_PKG_VERSION")
                            )),
                    ),
            )
            .child(
                div()
                    .max_w(rems(25.0))
                    .text_center()
                    .text_sm()
                    .text_color(theme.foreground)
                    .whitespace_normal()
                    .child(t(I18nKey::AboutDesc, lang)),
            )
            .child(
                div()
                    .pt_8()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(t(I18nKey::Copyright, lang)),
            )
    }

    fn render_path_item(
        &self,
        label: &str,
        desc: &str,
        input: &Entity<InputState>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let input_clone = input.clone();
        let label_text = label.to_string();
        v_flex()
            .gap_2()
            .w_full()
            .child(
                v_flex()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .whitespace_normal()
                            .child(desc.to_string()),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(setting_input(Input::new(input), theme).flex_grow())
                    .child(
                        Button::new(SharedString::from(format!("select-{label}")))
                            .child(Icon::new(IconName::FolderSelect).size(rems(0.875)))
                            .on_click(cx.listener(move |_, _, window, cx| {
                                let input_state = input_clone.clone();
                                let prompt_title = format!("选择{label_text}");
                                let window_handle = window.window_handle();
                                let receiver = cx.prompt_for_paths(PathPromptOptions {
                                    files: false,
                                    directories: true,
                                    multiple: false,
                                    prompt: Some(prompt_title.into()),
                                });
                                cx.spawn(move |_, cx: &mut AsyncApp| {
                                    let mut cx = cx.clone();
                                    async move {
                                        if let Ok(Ok(Some(paths))) = receiver.await
                                            && let Some(path) = paths.first()
                                        {
                                            let path_str = path.to_string_lossy().to_string();
                                            let _ =
                                                cx.update_window(window_handle, |_, window, cx| {
                                                    input_state.update(cx, |state, cx| {
                                                        let len = state.text().len();
                                                        state.replace_text_in_range(
                                                            Some(0..len),
                                                            &path_str,
                                                            window,
                                                            cx,
                                                        );
                                                    });
                                                });
                                        }
                                    }
                                })
                                .detach();
                            })),
                    ),
            )
    }

    fn sidebar_top_padding(&self) -> DefiniteLength {
        #[cfg(target_os = "macos")]
        return rems(2.5).into();
        #[cfg(not(target_os = "macos"))]
        return rems(2.0).into();
    }

    fn render_sidebar_drag_area(&self) -> Option<impl IntoElement> {
        #[cfg(not(target_os = "macos"))]
        return Some(
            div()
                .h(rems(2.0))
                .w_full()
                .absolute()
                .top_0()
                .left_0()
                .window_control_area(WindowControlArea::Drag),
        );
        #[cfg(target_os = "macos")]
        None::<gpui::Div>
    }

    fn render_content_drag_area(&self) -> Option<impl IntoElement> {
        #[cfg(not(target_os = "macos"))]
        return Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right(rems(6.25)) // 为窗口控件留空间
                .h(rems(2.0))
                .window_control_area(WindowControlArea::Drag),
        );
        #[cfg(target_os = "macos")]
        None::<gpui::Div>
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self
            .config
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();

        // 动态更新 EasyScholar 占位符以响应语言切换
        self.easyscholar_key_input.update(cx, |state, cx| {
            state.set_placeholder(t(I18nKey::EasyScholarPlaceholder, lang), window, cx);
        });

        #[cfg(not(target_os = "macos"))]
        let is_maximized = window.is_maximized();
        #[cfg(target_os = "macos")]
        let is_maximized = false;
        #[cfg(target_os = "macos")]
        let _ = window; // 避免未使用警告

        div()
            .size_full()
            .bg(theme.background)
            .flex()
            .child(
                v_flex()
                    .w(rems(12.5))
                    .flex_shrink_0()
                    .h_full()
                    .bg(theme.muted)
                    .border_r_1()
                    .border_color(theme.border)
                    .relative()
                    // 平台差异化处理
                    .pt(self.sidebar_top_padding())
                    .children(self.render_sidebar_drag_area())
                    .child(
                        v_flex()
                            .p_2()
                            .gap_1()
                            .child(self.render_sidebar_item(
                                SettingsTab::General,
                                Icon::new(IconName::Settings),
                                t(I18nKey::General, lang),
                                &theme,
                                cx,
                            ))
                            .child(self.render_sidebar_item(
                                SettingsTab::Sync,
                                Icon::new(IconName::Cloud),
                                t(I18nKey::Sync, lang),
                                &theme,
                                cx,
                            ))
                            .child(self.render_sidebar_item(
                                SettingsTab::AiBackends,
                                Icon::new(IconName::Puzzle),
                                t(I18nKey::AiBackendsSettingsTab, lang),
                                &theme,
                                cx,
                            ))
                            .child(self.render_sidebar_item(
                                SettingsTab::Translation,
                                Icon::new(IconName::Globe),
                                t(I18nKey::TranslationSettingsTab, lang),
                                &theme,
                                cx,
                            ))
                            .child(self.render_sidebar_item(
                                SettingsTab::AiChat,
                                Icon::new(IconName::BookOpen),
                                t(I18nKey::AiChatSettingsTab, lang),
                                &theme,
                                cx,
                            ))
                            .child(self.render_sidebar_item(
                                SettingsTab::About,
                                Icon::new(IconName::Info),
                                t(I18nKey::About, lang),
                                &theme,
                                cx,
                            )),
                    )
                    .child(div().flex_grow())
                    .child(
                        v_flex()
                            .p_3()
                            .border_t_1()
                            .border_color(theme.border.opacity(0.5))
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("settings-cancel")
                                            .child(t(I18nKey::Cancel, lang))
                                            .small()
                                            .w(rems(4.5))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.handle_cancel(window, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("settings-save")
                                            .child(t(I18nKey::Save, lang))
                                            .primary()
                                            .small()
                                            .w(rems(4.5))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.handle_save(window, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w(rems(0.0))
                    .bg(theme.background)
                    .relative()
                    // Windows: 窗口控件
                    .when(cfg!(not(target_os = "macos")), |this| {
                        this.child(self.render_window_controls(&theme, is_maximized))
                    })
                    // Windows: 顶部拖动区域
                    .children(self.render_content_drag_area())
                    .child(div().size_full().overflow_y_scrollbar().p_8().child(
                        match self.active_tab {
                            SettingsTab::General => {
                                self.render_general_tab(&theme, cx).into_any_element()
                            }
                            SettingsTab::Sync => {
                                self.render_sync_tab(&theme, cx).into_any_element()
                            }
                            SettingsTab::AiBackends => {
                                self.render_ai_backends_tab(&theme, cx).into_any_element()
                            }
                            SettingsTab::Translation => {
                                self.render_translation_tab(&theme, cx).into_any_element()
                            }
                            SettingsTab::AiChat => {
                                self.render_ai_chat_tab(&theme, cx).into_any_element()
                            }
                            SettingsTab::About => self.render_about_tab(&theme).into_any_element(),
                        },
                    )),
            )
            .child(self.toast_overlay.clone())
    }
}

impl SettingsWindow {
    /// 渲染 Windows 窗口控件
    #[cfg(not(target_os = "macos"))]
    fn render_window_controls(&self, theme: &Theme, is_maximized: bool) -> impl IntoElement {
        h_flex()
            .absolute()
            .top_1()
            .right_1()
            .items_center()
            .gap_0p5()
            // 最小化按钮
            .child(
                div()
                    .id("settings-window-minimize")
                    .h(rems(1.5))
                    .w(rems(1.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .occlude()
                    .window_control_area(WindowControlArea::Min)
                    .hover(|s| s.bg(theme.muted_foreground.opacity(0.2)))
                    .child(
                        Icon::new(IconName::Minimize)
                            .size(rems(0.875))
                            .text_color(theme.foreground),
                    ),
            )
            // 最大化/还原按钮
            .child(
                div()
                    .id("settings-window-maximize-restore")
                    .h(rems(1.5))
                    .w(rems(1.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .occlude()
                    .window_control_area(WindowControlArea::Max)
                    .hover(|s| s.bg(theme.muted_foreground.opacity(0.2)))
                    .child(
                        Icon::new(if is_maximized {
                            IconName::Restore
                        } else {
                            IconName::Maximize
                        })
                        .size(rems(0.875))
                        .text_color(theme.foreground),
                    ),
            )
            // 关闭按钮
            .child(
                div()
                    .id("settings-window-close")
                    .h(rems(1.5))
                    .w(rems(1.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .occlude()
                    .window_control_area(WindowControlArea::Close)
                    .hover(|s| s.bg(gpui::red().opacity(0.9)))
                    .child(
                        Icon::new(IconName::Close)
                            .size(rems(0.875))
                            .text_color(theme.foreground),
                    ),
            )
    }

    #[cfg(target_os = "macos")]
    fn render_window_controls(&self, _theme: &Theme, _is_maximized: bool) -> impl IntoElement {
        div()
    }
}
