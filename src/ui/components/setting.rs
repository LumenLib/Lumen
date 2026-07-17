use crate::config::AppConfig;
use crate::config_store::ConfigStore;
use crate::services::{MainApp, utils::filename};
use crate::ui::theme_manager::{LOADER, surface};
use crate::ui::views::main_window::utils::open_url;
use components::IconName;
use components::{muted_input, selector};
use gpui::prelude::*;
use gpui::{
    App, AppContext, AsyncApp, Entity, EntityInputHandler, InteractiveElement, MouseButton,
    PathPromptOptions, SharedString, Window, WindowId, div, px, rems, transparent_black,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    switch::Switch,
    v_flex,
};
use i18n::{I18nKey, Language, t};
use log::{error, info};
use std::sync::Arc;
use translate;

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

/// 设置窗口视图
pub struct SettingsWindow {
    app: Arc<MainApp>,
    initial_config: AppConfig,
    saved_flag: Arc<std::sync::atomic::AtomicBool>,
    _close_subscription: Option<gpui::Subscription>,
    toast_overlay: Entity<crate::ui::components::ToastOverlay>,
    initial_tab: Option<SettingsTab>,

    // Test results
    webdav_tested: bool,
    webdav_test_result: Option<Result<(), String>>,
    db_tested: bool,
    db_test_result: Option<Result<(), String>>,

    // AI Backends state
    ai_entries: Vec<translate::AiBackendEntry>,
    ai_edit_target: Option<usize>,
    ai_adding_new: bool,
    ai_edit_name_input: Entity<InputState>,
    ai_edit_kind_value: SharedString,
    ai_edit_api_key_input: Entity<InputState>,
    ai_edit_api_base_input: Entity<InputState>,
    ai_edit_model_input: Entity<InputState>,
    ai_edit_context_window_input: Entity<InputState>,
    ai_edit_compression_strategy_value: SharedString,
    ai_edit_enable_thinking: bool,
}

// ─── Helpers ──────────────────────────────────────────────────────

fn config_str<F: Fn(&AppConfig) -> &String + Copy + 'static>(
    f: F,
) -> impl Fn(&App) -> SharedString + Copy {
    move |cx| f(&cx.global::<ConfigStore>().inner).clone().into()
}

fn set_config_str<F: Fn(&mut AppConfig) -> &mut String + Copy + 'static>(
    f: F,
) -> impl Fn(SharedString, &mut App) + Copy {
    move |v, cx| {
        cx.update_global::<ConfigStore, _>(|store, _| {
            *f(&mut store.inner) = v.into();
        });
    }
}

fn config_bool<F: Fn(&AppConfig) -> bool + Copy + 'static>(f: F) -> impl Fn(&App) -> bool + Copy {
    move |cx| f(&cx.global::<ConfigStore>().inner)
}

fn set_config_bool<F: Fn(&mut AppConfig) -> &mut bool + Copy + 'static>(
    f: F,
) -> impl Fn(bool, &mut App) + Copy {
    move |v, cx| {
        cx.update_global::<ConfigStore, _>(|store, _| {
            *f(&mut store.inner) = v;
        });
    }
}

fn lang(cx: &App) -> Language {
    cx.global::<ConfigStore>().current_language()
}

/// Render a path picker: input + browse button + async file dialog
fn path_picker_element(
    id: &'static str,
    get: impl Fn(&App) -> SharedString + 'static,
    set: impl Fn(SharedString, &mut App) + 'static,
    browse_label: SharedString,
    prompt: SharedString,
    title: &'static str,
    desc: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let key = SharedString::from(format!("path-{id}"));

    struct PickerState {
        input: Entity<InputState>,
        _sub: gpui::Subscription,
    }

    let state_entity = window.use_keyed_state::<PickerState>(key, cx, |w, cx| {
        let initial = get(cx);
        let input = cx.new(|cx| InputState::new(w, cx).default_value(initial));
        let _sub = cx.subscribe(&input, move |_, emitter, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                let v = emitter.read(cx).value();
                set(v, cx);
            }
        });
        PickerState { input, _sub }
    });

    let theme = cx.theme();
    let state = state_entity.read(cx);

    v_flex()
        .gap_1()
        .child(
            Label::new(title)
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD),
        )
        .when(!desc.is_empty(), |this| {
            this.child(
                Label::new(desc)
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
        })
        .child(
            h_flex()
                .gap_2()
                .child(muted_input(Input::new(&state.input), theme).flex_grow(1.0))
                .child(
                    Button::new(SharedString::from(format!("browse-{id}")))
                        .icon(IconName::FolderSelect)
                        .tooltip(browse_label)
                        .on_click({
                            let input = state.input.clone();
                            let prompt_str = prompt.clone();
                            move |_, window, cx| {
                                let handle = window.window_handle();
                                let receiver = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: true,
                                    multiple: false,
                                    prompt: Some(prompt_str.clone()),
                                });
                                let input = input.clone();
                                cx.spawn(move |cx: &mut AsyncApp| {
                                    let mut cx = cx.clone();
                                    async move {
                                        if let Ok(Ok(Some(paths))) = receiver.await
                                            && let Some(path) = paths.first()
                                        {
                                            let path_str = path.to_string_lossy().to_string();
                                            let _ = cx.update_window(handle, |_, window, cx| {
                                                input.update(cx, |state, cx| {
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
                            }
                        }),
                ),
        )
}

// ─── SettingsWindow ───────────────────────────────────────────────

impl SettingsWindow {
    pub fn new(
        app: Arc<MainApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
        initial_tab: Option<SettingsTab>,
    ) -> Self {
        let config = cx.global::<ConfigStore>().inner.clone();
        let saved_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let close_subscription = Some(cx.on_window_closed({
            let app = app.clone();
            let initial_config = config.clone();
            let saved_flag = saved_flag.clone();
            move |cx: &mut App, _: WindowId| {
                // 只有在未主动保存/取消的情况下才恢复（窗口被强制关闭）
                if !saved_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    cx.set_global(ConfigStore {
                        inner: initial_config.clone(),
                    });
                    if let Err(e) = app.update_config(initial_config.clone()) {
                        error!("恢复配置失败: {e}");
                    }
                }
            }
        }));

        let translation_keys = app.local_state.read().unwrap().translation_keys.clone();
        let ai_entries: Vec<translate::AiBackendEntry> = translation_keys
            .get("ai.entries")
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        let ai_edit_name_input = cx.new(|cx| InputState::new(window, cx));
        let ai_edit_api_key_input = cx.new(|cx| InputState::new(window, cx).masked(true));
        let ai_edit_api_base_input = cx.new(|cx| InputState::new(window, cx));
        let ai_edit_model_input = cx.new(|cx| InputState::new(window, cx));
        let ai_edit_context_window_input = cx.new(|cx| InputState::new(window, cx));

        Self {
            app,
            initial_config: config,
            saved_flag,
            _close_subscription: close_subscription,
            toast_overlay: cx.new(|cx| crate::ui::components::ToastOverlay::new(window, cx)),
            initial_tab,

            webdav_tested: false,
            webdav_test_result: None,
            db_tested: false,
            db_test_result: None,

            ai_entries,
            ai_edit_target: None,
            ai_adding_new: false,
            ai_edit_name_input,
            ai_edit_kind_value: "openai".into(),
            ai_edit_api_key_input,
            ai_edit_api_base_input,
            ai_edit_model_input,
            ai_edit_context_window_input,
            ai_edit_compression_strategy_value: "none".into(),
            ai_edit_enable_thinking: false,
        }
    }

    fn handle_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("设置窗口: 开始保存配置");
        self.saved_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let new_config = cx.global::<ConfigStore>().inner.clone();

        // Persist translation keys + AI entries + password from local_state
        if let Ok(mut state) = self.app.local_state.write() {
            state.translation_keys.insert(
                "ai.entries".to_string(),
                serde_json::to_string(&self.ai_entries).unwrap_or_default(),
            );
            let _ = self.app.local_state_manager.save_all(&state);
        }

        cx.set_global(ConfigStore {
            inner: new_config.clone(),
        });
        self.app.notify_ui_changed();

        if let Err(e) = self.app.update_config(new_config) {
            error!("更新配置失败: {e}");
        } else {
            info!("设置窗口: 配置已保存");
            window.remove_window();
        }
    }

    fn handle_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("设置窗口: 取消并还原配置");
        self.saved_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);
        cx.set_global(ConfigStore {
            inner: self.initial_config.clone(),
        });
        if let Err(e) = self.app.update_config(self.initial_config.clone()) {
            error!("恢复配置失败: {e}");
        }
        window.remove_window();
    }

    fn pages(&self, _window: &mut Window, cx: &mut Context<Self>) -> Vec<SettingPage> {
        let app = self.app.clone();

        vec![
            self.general_page(app.clone(), cx),
            self.sync_page(app.clone(), cx),
            self.ai_backends_page(app.clone(), cx),
            self.translation_page(app.clone(), cx),
            self.ai_chat_page(app.clone(), cx),
            self.about_page(cx),
        ]
    }

    // ── General Page ───────────────────────────────────────────────

    fn general_page(&self, app: Arc<MainApp>, cx: &mut Context<Self>) -> SettingPage {
        let l = lang(cx);

        // ── Dropdown option builders ───────────────────────────────

        let lang_options: Vec<(SharedString, SharedString)> = [
            (Language::ZhCn, "简体中文"),
            (Language::ZhTw, "繁體中文"),
            (Language::En, "English"),
            (Language::Ja, "日本語"),
            (Language::Ko, "한국어"),
            (Language::Ru, "Русский"),
            (Language::Fr, "Français"),
            (Language::De, "Deutsch"),
            (Language::Es, "Español"),
        ]
        .iter()
        .map(|(l, label)| (l.as_str().to_string().into(), (*label).into()))
        .collect();

        let scale_options: Vec<(SharedString, SharedString)> = (0..=12)
            .map(|i| {
                let v = 0.8 + i as f32 * 0.1;
                (
                    format!("{v:.1}").into(),
                    format!("{}%", (v * 100.0) as u32).into(),
                )
            })
            .collect();

        let log_options: Vec<(SharedString, SharedString)> = [
            ("debug", "Debug"),
            ("info", "Info"),
            ("warn", "Warn"),
            ("error", "Error"),
        ]
        .iter()
        .map(|(v, l)| ((*v).into(), (*l).into()))
        .collect();

        let notif_options: Vec<(SharedString, SharedString)> =
            [("all", "All"), ("warn", "Warn"), ("error", "Error")]
                .iter()
                .map(|(v, l)| ((*v).into(), (*l).into()))
                .collect();

        let mut theme_style_options: Vec<(SharedString, SharedString)> =
            vec![("default".into(), "Default".into())];
        if let Ok(loader) = LOADER.read() {
            for name in loader.available_themes() {
                theme_style_options.push((name.clone().into(), name.into()));
            }
        }

        // ── Library Settings group ─────────────────────────────────

        let app_for_lib = app.clone();
        let library_group = {
            SettingGroup::new()
                .title(t(I18nKey::LibrarySettings, l))
                .item(SettingItem::render({
                    move |_, window, cx| {
                        let l = lang(cx);
                        path_picker_element(
                            "base-dir",
                            |cx| {
                                cx.global::<ConfigStore>()
                                    .inner
                                    .base_dir
                                    .to_string_lossy()
                                    .to_string()
                                    .into()
                            },
                            |v, cx| {
                                cx.update_global::<ConfigStore, _>(|store, _| {
                                    store.inner.base_dir = v.to_string().into();
                                });
                            },
                            "Browse...".into(),
                            t(I18nKey::DatabaseDir, l).into(),
                            t(I18nKey::DatabaseDir, l),
                            t(I18nKey::DatabaseDirDesc, l),
                            window,
                            cx,
                        )
                        .into_any_element()
                    }
                }))
                .item(SettingItem::render({
                    move |_, window, cx| {
                        let l = lang(cx);
                        path_picker_element(
                            "attachment-path",
                            |cx| {
                                cx.global::<ConfigStore>()
                                    .inner
                                    .attachment_path
                                    .to_string_lossy()
                                    .to_string()
                                    .into()
                            },
                            |v, cx| {
                                cx.update_global::<ConfigStore, _>(|store, _| {
                                    store.inner.attachment_path = v.to_string().into();
                                });
                            },
                            "Browse...".into(),
                            t(I18nKey::AttachmentDir, l).into(),
                            t(I18nKey::AttachmentDir, l),
                            t(I18nKey::AttachmentDirDesc, l),
                            window,
                            cx,
                        )
                        .into_any_element()
                    }
                }))
                .item(SettingItem::render({
                    let app = app_for_lib.clone();
                    move |_, p_window, cx| {
                        let l = lang(cx);

                        // Build input state first, before borrowing theme
                        struct FnState {
                            input: Entity<InputState>,
                            _sub: gpui::Subscription,
                        }
                        let template = cx.global::<ConfigStore>().inner.filename_template.clone();
                        let state = p_window.use_keyed_state::<FnState>(
                            "filename-template-input",
                            cx,
                            |window, cx| {
                                let input = cx
                                    .new(|cx| InputState::new(window, cx).default_value(template));
                                let _sub = cx.subscribe(&input, {
                                    move |_, emitter, event: &InputEvent, cx| {
                                        if let InputEvent::Change = event {
                                            let v = emitter.read(cx).value();
                                            cx.update_global::<ConfigStore, _>(|store, _| {
                                                store.inner.filename_template = v.to_string();
                                            });
                                        }
                                    }
                                });
                                FnState { input, _sub }
                            },
                        );

                        let theme = cx.theme();
                        let preview_options = filename::FilenameOptions::new(
                            "He",
                            "Kaiming",
                            "2022",
                            "Masked Autoencoders Are Scalable Vision Learners",
                            "CVPR",
                            "pdf",
                            true,
                        );
                        let active_template =
                            cx.global::<ConfigStore>().inner.filename_template.clone();
                        let preview_name = filename::generate_filename_from_template(
                            &active_template,
                            &preview_options,
                        );

                        v_flex()
                            .gap_2()
                            .w_full()
                            .child(
                                v_flex()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child(t(I18nKey::FilenameTemplate, l)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .whitespace_normal()
                                            .child(t(I18nKey::FilenameTemplateDesc, l)),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        muted_input(Input::new(&state.read(cx).input), theme)
                                            .flex_grow(1.0),
                                    )
                                    .child(
                                        Button::new("batch-rename")
                                            .child(t(I18nKey::BatchRename, l))
                                            .w(rems(4.5))
                                            .on_click({
                                                let app = app.clone();
                                                move |_, _, cx| {
                                                    let app = app.clone();
                                                    cx.spawn(move |_: &mut AsyncApp| {
                                                        let app = app.clone();
                                                        async move {
                                                            if let Err(e) = app.batch_rename_files()
                                                            {
                                                                log::error!("批量重命名失败: {e}");
                                                            }
                                                        }
                                                    })
                                                    .detach();
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new("cleanup-orphaned")
                                            .child(t(I18nKey::CleanupOrphanedFiles, l))
                                            .w(rems(4.5))
                                            .on_click({
                                                let app = app.clone();
                                                move |_, _, cx| {
                                                    let app = app.clone();
                                                    cx.spawn(move |_: &mut AsyncApp| {
                                                        let app = app.clone();
                                                        async move {
                                                            if let Err(e) =
                                                                app.cleanup_orphaned_files()
                                                            {
                                                                log::error!(
                                                                    "清理孤立文件失败: {e}"
                                                                );
                                                            }
                                                        }
                                                    })
                                                    .detach();
                                                }
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(format!("{}: ", t(I18nKey::Preview, l))),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.primary)
                                            .child(preview_name),
                                    ),
                            )
                            .into_any_element()
                    }
                }))
                .item(SettingItem::render({
                    let app = app.clone();
                    move |_, p_window, cx| {
                        let l = lang(cx);
                        let current_val = app
                            .sync_service
                            .db
                            .get_sync_meta("easyscholar_key")
                            .ok()
                            .flatten()
                            .unwrap_or_default();

                        struct EsState {
                            input: Entity<InputState>,
                            _sub: gpui::Subscription,
                        }
                        let state = p_window.use_keyed_state::<EsState>(
                            "easyscholar-key-input",
                            cx,
                            |window, cx| {
                                let input = cx.new(|cx| {
                                    InputState::new(window, cx)
                                        .default_value(current_val)
                                        .placeholder(t(I18nKey::EasyScholarPlaceholder, l))
                                });
                                let app = app.clone();
                                let _sub = cx.subscribe(&input, {
                                    move |_, emitter, event: &InputEvent, cx| {
                                        if let InputEvent::Change = event {
                                            let v = emitter.read(cx).value();
                                            let _ = app
                                                .sync_service
                                                .db
                                                .set_sync_meta("easyscholar_key", &v);
                                        }
                                    }
                                });
                                EsState { input, _sub }
                            },
                        );
                        // 动态更新占位符以响应语言切换（在 theme 借用前完成）
                        let es_input = state.read(cx).input.clone();
                        let current_l = lang(cx);
                        es_input.update(cx, |s, cx| {
                            s.set_placeholder(
                                t(I18nKey::EasyScholarPlaceholder, current_l),
                                p_window,
                                cx,
                            );
                        });
                        let theme = cx.theme();
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(t(I18nKey::MetadataServices, l)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(t(I18nKey::EasyScholarDesc, l)),
                            )
                            .child(muted_input(Input::new(&state.read(cx).input), theme))
                            .into_any_element()
                    }
                }))
        };

        // ── PDF Viewer Settings group ──────────────────────────────

        let pdf_group = {
            SettingGroup::new()
                .title(t(I18nKey::PdfViewerSettings, l))
                .item(SettingItem::new(
                    t(I18nKey::UseCustomPdfViewer, l),
                    SettingField::switch(
                        config_bool(|c| c.pdf_viewer.use_custom),
                        set_config_bool(|c| &mut c.pdf_viewer.use_custom),
                    ),
                ))
                .item(SettingItem::render(move |_, window, cx| {
                    let enabled = cx.global::<ConfigStore>().pdf_viewer.use_custom;
                    if !enabled {
                        return div().into_any_element();
                    }

                    let cfg_get = config_str(|c| &c.pdf_viewer.macos_app);
                    let cfg_set = set_config_str(|c| &mut c.pdf_viewer.macos_app);

                    v_flex()
                        .gap_4()
                        .when(cfg!(target_os = "macos"), |this| {
                            this.child(path_picker_element(
                                "pdf-macos",
                                move |cx| cfg_get(cx),
                                move |v, cx| cfg_set(v, cx),
                                t(I18nKey::SelectMacosPdfReader, lang(cx)).into(),
                                t(I18nKey::SelectMacosPdfReader, lang(cx)).into(),
                                t(I18nKey::PdfViewerPathMacos, lang(cx)),
                                "",
                                window,
                                cx,
                            ))
                        })
                        .when(cfg!(target_os = "windows"), |this| {
                            this.child(path_picker_element(
                                "pdf-windows",
                                config_str(|c| &c.pdf_viewer.windows_app),
                                set_config_str(|c| &mut c.pdf_viewer.windows_app),
                                t(I18nKey::SelectWindowsPdfReader, lang(cx)).into(),
                                t(I18nKey::SelectWindowsPdfReader, lang(cx)).into(),
                                t(I18nKey::PdfViewerPathWindows, lang(cx)),
                                "",
                                window,
                                cx,
                            ))
                        })
                        .into_any_element()
                }))
        };

        // ── Proxy group ────────────────────────────────────────────

        let proxy_group = {
            SettingGroup::new()
                .title(t(I18nKey::NetworkProxySettings, l))
                .item(SettingItem::new(
                    t(I18nKey::EnableProxyServer, l),
                    SettingField::switch(
                        config_bool(|c| c.proxy.enabled),
                        set_config_bool(|c| &mut c.proxy.enabled),
                    ),
                ))
                .item(SettingItem::render(move |_, window, cx| {
                    struct ProxyState {
                        input: Entity<InputState>,
                        _sub: gpui::Subscription,
                    }
                    let enabled = cx.global::<ConfigStore>().proxy.enabled;
                    if !enabled {
                        return div().into_any_element();
                    }
                    let val = cx.global::<ConfigStore>().proxy.url.clone();
                    let state = window.use_keyed_state::<ProxyState>(
                        "proxy-url-input",
                        cx,
                        |window, cx| {
                            let input = cx.new(|cx| InputState::new(window, cx).default_value(val));
                            let _sub = cx.subscribe(&input, {
                                move |_, emitter, event: &InputEvent, cx| {
                                    if let InputEvent::Change = event {
                                        let v = emitter.read(cx).value();
                                        cx.update_global::<ConfigStore, _>(|store, _| {
                                            store.inner.proxy.url = v.into();
                                        });
                                    }
                                }
                            });
                            ProxyState { input, _sub }
                        },
                    );
                    let theme = cx.theme();
                    h_flex()
                        .gap_2()
                        .child(muted_input(Input::new(&state.read(cx).input), theme).flex_grow(1.0))
                        .into_any_element()
                }))
        };

        // ── Assemble General page ──────────────────────────────────

        SettingPage::new(t(I18nKey::General, l))
            .icon(Icon::new(IconName::Settings))
            .group(
                SettingGroup::new()
                    .title(t(I18nKey::GeneralOptions, l))
                    .item(SettingItem::render({
                        let app = app.clone();
                        let lang_options = lang_options.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let current = config_str(|c| &c.ui.language)(cx);
                            let app_clone = app.clone();
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(t(I18nKey::Language, l)),
                                )
                                .child(selector(
                                    "lang-select",
                                    lang_options.clone(),
                                    current,
                                    true,
                                    move |v, _, cx| {
                                        cx.update_global::<ConfigStore, _>(|store, _| {
                                            store.inner.ui.language = v.to_string();
                                        });
                                        let _ = app_clone.update_config(
                                            cx.global::<ConfigStore>().inner.clone(),
                                        );
                                    },
                                ))
                        }
                    }))
                    .item(SettingItem::render({
                        let app = app.clone();
                        let theme_style_options = theme_style_options.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let current = config_str(|c| &c.ui.theme_style)(cx);
                            let app_clone = app.clone();
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(t(I18nKey::ThemeStyle, l)),
                                )
                                .child(selector(
                                    "theme-style-select",
                                    theme_style_options.clone(),
                                    current,
                                    false,
                                    move |v, _, cx| {
                                        cx.update_global::<ConfigStore, _>(|store, _| {
                                            store.inner.ui.theme_style = v.to_string();
                                        });
                                        let _ = app_clone.update_config(
                                            cx.global::<ConfigStore>().inner.clone(),
                                        );
                                    },
                                ))
                        }
                    }))
                    .item(SettingItem::render({
                        let app = app.clone();
                        let scale_options = scale_options.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let current =
                                format!("{:.1}", cx.global::<ConfigStore>().ui.ui_scale).into();
                            let app_clone = app.clone();
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(t(I18nKey::UiScale, l)),
                                )
                                .child(selector(
                                    "ui-scale-select",
                                    scale_options.clone(),
                                    current,
                                    false,
                                    move |v, _, cx| {
                                        if let Ok(scale) = v.parse::<f32>() {
                                            cx.update_global::<ConfigStore, _>(|store, _| {
                                                store.inner.ui.ui_scale = scale;
                                            });
                                            let _ = app_clone.update_config(
                                                cx.global::<ConfigStore>().inner.clone(),
                                            );
                                        }
                                    },
                                ))
                        }
                    }))
                    .item(SettingItem::render({
                        let app = app.clone();
                        let log_options = log_options.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let current = config_str(|c| &c.log_level)(cx);
                            let app_clone = app.clone();
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(t(I18nKey::LogLevel, l)),
                                )
                                .child(selector(
                                    "log-level-select",
                                    log_options.clone(),
                                    current,
                                    false,
                                    move |v, _, cx| {
                                        cx.update_global::<ConfigStore, _>(|store, _| {
                                            store.inner.log_level = v.to_string();
                                        });
                                        let _ = app_clone.update_config(
                                            cx.global::<ConfigStore>().inner.clone(),
                                        );
                                    },
                                ))
                        }
                    }))
                    .item(SettingItem::render({
                        let app = app.clone();
                        let notif_options = notif_options.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let current = config_str(|c| &c.notification_level)(cx);
                            let app_clone = app.clone();
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .child(t(I18nKey::NotificationLevel, l)),
                                )
                                .child(selector(
                                    "notif-level-select",
                                    notif_options.clone(),
                                    current,
                                    false,
                                    move |v, _, cx| {
                                        cx.update_global::<ConfigStore, _>(|store, _| {
                                            store.inner.notification_level = v.to_string();
                                        });
                                        let _ = app_clone.update_config(
                                            cx.global::<ConfigStore>().inner.clone(),
                                        );
                                    },
                                ))
                        }
                    }))
                    .item(SettingItem::render({
                        let app = app.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let theme = cx.theme();
                            let current_mode = config_str(|c| &c.ui.theme_mode)(cx);

                            let mk = |_id: &'static str, val: &'static str, label: SharedString| {
                                let app = app.clone();
                                let is_active = current_mode == val;
                                div()
                                    .px_3()
                                    .py_1()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if is_active {
                                        surface().chip_bg
                                    } else {
                                        transparent_black()
                                    })
                                    .text_color(if is_active {
                                        theme.foreground
                                    } else {
                                        theme.foreground
                                    })
                                    .text_sm()
                                    .on_mouse_down(MouseButton::Left, {
                                        move |_, _, cx| {
                                            cx.update_global::<ConfigStore, _>(|store, _| {
                                                store.inner.ui.theme_mode = val.to_string();
                                            });
                                            let _ = app.update_config(
                                                cx.global::<ConfigStore>().inner.clone(),
                                            );
                                        }
                                    })
                                    .child(label)
                            };

                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD)
                                                .child(t(I18nKey::Appearance, l)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(theme.muted_foreground)
                                                .child(t(I18nKey::ThemeDesc, l)),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .p_1()
                                        .bg(theme.muted)
                                        .rounded_md()
                                        .child(mk(
                                            "theme-light",
                                            "light",
                                            t(I18nKey::Light, l).into(),
                                        ))
                                        .child(mk("theme-dark", "dark", t(I18nKey::Dark, l).into()))
                                        .child(mk(
                                            "theme-system",
                                            "system",
                                            t(I18nKey::System, l).into(),
                                        )),
                                )
                        }
                    })),
            )
            .group(library_group)
            .group(pdf_group)
            .group(proxy_group)
    }

    // ── Translation Page ───────────────────────────────────────────

    fn translation_page(&self, app: Arc<MainApp>, cx: &mut Context<Self>) -> SettingPage {
        let l = lang(cx);
        let engines: Vec<(SharedString, SharedString)> = [
            ("google_free", "Google (Free)"),
            ("bing_free", "Bing (Free)"),
            ("google", "Google"),
            ("niutrans", "NiuTrans"),
            ("baidu", "Baidu"),
            ("youdao", "Youdao"),
            ("deepl_free", "DeepL (Free)"),
            ("deepl_pro", "DeepL (Pro)"),
            ("ai", "AI"),
        ]
        .iter()
        .map(|(v, l)| ((*v).into(), (*l).into()))
        .collect();

        let target_lang_options: Vec<(SharedString, SharedString)> = [
            ("zh-CN", "简体中文"),
            ("zh-TW", "繁體中文"),
            ("en", "English"),
            ("ja", "日本語"),
            ("ko", "한국어"),
            ("fr", "Français"),
            ("de", "Deutsch"),
            ("es", "Español"),
            ("ru", "Русский"),
            ("pt", "Português"),
            ("it", "Italiano"),
            ("nl", "Nederlands"),
            ("ar", "العربية"),
            ("th", "ไทย"),
            ("vi", "Tiếng Việt"),
        ]
        .iter()
        .map(|(v, l)| ((*v).into(), (*l).into()))
        .collect();

        // Conditional API key input helper
        let api_key_field =
            |key_id: &'static str, label: &'static str, key_s: &'static str| -> SettingItem {
                let app = app.clone();
                SettingItem::render(move |_, window, cx| {
                    let engine = cx.global::<ConfigStore>().translation.engine.clone();
                    let show = translate::ENGINES
                        .iter()
                        .any(|e| e.id == engine && e.requires_keys.contains(&key_s));
                    if !show {
                        return div().into_any_element();
                    }
                    struct KeyState {
                        input: Entity<InputState>,
                        _sub: gpui::Subscription,
                    }
                    let val: SharedString = app
                        .local_state
                        .read()
                        .unwrap()
                        .translation_keys
                        .get(key_s)
                        .cloned()
                        .unwrap_or_default()
                        .into();
                    let state = window.use_keyed_state::<KeyState>(
                        format!("tkey-{key_id}"),
                        cx,
                        |window, cx| {
                            let input = cx.new(|cx| {
                                InputState::new(window, cx).default_value(val).masked(true)
                            });
                            let app = app.clone();
                            let k = key_s.to_string();
                            let _sub = cx.subscribe(&input, {
                                move |_, emitter, event: &InputEvent, cx| {
                                    if let InputEvent::Change = event {
                                        let v = emitter.read(cx).value();
                                        let mut state = app.local_state.write().unwrap();
                                        state.translation_keys.insert(k.clone(), v.to_string());
                                    }
                                }
                            });
                            KeyState { input, _sub }
                        },
                    );
                    let theme = cx.theme();
                    muted_input(Input::new(&state.read(cx).input), theme)
                        .child(
                            Label::new(label)
                                .text_xs()
                                .text_color(theme.muted_foreground),
                        )
                        .into_any_element()
                })
            };

        let base =
            SettingPage::new(t(I18nKey::TranslationSettingsTab, l))
                .icon(Icon::new(IconName::Globe))
                .group(
                    SettingGroup::new()
                        .title(t(I18nKey::TranslationSettings, l))
                        .item(SettingItem::render({
                            let engines = engines.clone();
                            let app = app.clone();
                            move |_, _, cx| {
                                let l = lang(cx);
                                let current = config_str(|c| &c.translation.engine)(cx);
                                let app_clone = app.clone();
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child(t(I18nKey::TranslationEngine, l)),
                                    )
                                    .child(selector(
                                        "translation-engine-select",
                                        engines.clone(),
                                        current,
                                        false,
                                        move |v, _, cx| {
                                            cx.update_global::<ConfigStore, _>(|store, _| {
                                                store.inner.translation.engine = v.to_string();
                                            });
                                            let _ = app_clone.update_config(
                                                cx.global::<ConfigStore>().inner.clone(),
                                            );
                                        },
                                    ))
                            }
                        }))
                        .item(SettingItem::render({
                            let target_lang_options = target_lang_options.clone();
                            let app = app.clone();
                            move |_, _, cx| {
                                let l = lang(cx);
                                let current = config_str(|c| &c.translation.target_language)(cx);
                                let app_clone = app.clone();
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child(t(I18nKey::TargetLanguage, l)),
                                    )
                                    .child(selector(
                                        "target-language-select",
                                        target_lang_options.clone(),
                                        current,
                                        false,
                                        move |v, _, cx| {
                                            cx.update_global::<ConfigStore, _>(|store, _| {
                                                store.inner.translation.target_language =
                                                    v.to_string();
                                            });
                                            let _ = app_clone.update_config(
                                                cx.global::<ConfigStore>().inner.clone(),
                                            );
                                        },
                                    ))
                            }
                        }))
                        .item(SettingItem::render({
                            let app = app.clone();
                            move |_, _, cx| {
                                let engine = cx.global::<ConfigStore>().translation.engine.clone();
                                if engine != "ai" {
                                    return div().into_any_element();
                                }
                                let (opts, active): (
                                    Vec<(SharedString, SharedString)>,
                                    SharedString,
                                ) = {
                                    let keys = app.local_state.read().unwrap();
                                    let json = keys
                                        .translation_keys
                                        .get("ai.entries")
                                        .cloned()
                                        .unwrap_or_default();
                                    let active = keys
                                        .translation_keys
                                        .get("ai.active")
                                        .cloned()
                                        .unwrap_or_default();
                                    drop(keys);
                                    let entries: Vec<translate::AiBackendEntry> =
                                        serde_json::from_str(&json).unwrap_or_default();
                                    let opts = entries
                                        .iter()
                                        .map(|e| (e.name.clone().into(), e.name.clone().into()))
                                        .collect();
                                    (opts, active.into())
                                };
                                if opts.is_empty() {
                                    return div().into_any_element();
                                }
                                let l = lang(cx);
                                let app_clone = app.clone();
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div().text_sm().font_weight(gpui::FontWeight::BOLD).child(
                                            if l == Language::ZhCn {
                                                "AI 后端"
                                            } else {
                                                "AI Backend"
                                            },
                                        ),
                                    )
                                    .child(selector(
                                        "translation-ai-active-select",
                                        opts,
                                        active,
                                        false,
                                        move |v, _, _cx| {
                                            let mut state = app_clone.local_state.write().unwrap();
                                            state
                                                .translation_keys
                                                .insert("ai.active".into(), v.to_string());
                                            let _ = app_clone.local_state_manager.save_all(&state);
                                        },
                                    ))
                                    .into_any_element()
                            }
                        }))
                        .item(api_key_field("google", "Google API Key", "google"))
                        .item(api_key_field("niutrans", "NiuTrans API Key", "niutrans"))
                        .item(api_key_field("baidu", "Baidu API Key", "baidu"))
                        .item(api_key_field("youdao", "Youdao API Key", "youdao"))
                        .item(api_key_field("deepl", "DeepL API Key", "deepl"))
                        .item(SettingItem::render(move |_, _, cx| {
                            let engine = cx.global::<ConfigStore>().translation.engine.clone();
                            let is_free = translate::ENGINES
                                .iter()
                                .any(|e| e.id == engine && e.is_free);
                            if !is_free {
                                return div().into_any_element();
                            }
                            let theme = cx.theme();
                            div()
                                .p_3()
                                .bg(surface().info_bg)
                                .rounded_md()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t(I18nKey::NoApiKeyRequired, lang(cx)))
                                .into_any_element()
                        })),
                );

        base
    }

    // ── AI Chat Page ───────────────────────────────────────────────

    fn ai_chat_page(&self, app: Arc<MainApp>, cx: &mut Context<Self>) -> SettingPage {
        let l = lang(cx);
        let (ai_sel_opts, ai_sel_active): (Vec<(SharedString, SharedString)>, SharedString) = {
            let keys = app.local_state.read().unwrap();
            let json = keys
                .translation_keys
                .get("ai.entries")
                .cloned()
                .unwrap_or_default();
            let active = keys
                .translation_keys
                .get("chat.active")
                .cloned()
                .unwrap_or_default();
            drop(keys);
            let entries: Vec<translate::AiBackendEntry> =
                serde_json::from_str(&json).unwrap_or_default();
            let opts = entries
                .iter()
                .map(|e| (e.name.clone().into(), e.name.clone().into()))
                .collect();
            (opts, active.into())
        };

        // System prompt textarea
        let sys_prompt = {
            let app = app.clone();
            SettingItem::render(move |_, window, cx| {
                struct PromptState {
                    input: Entity<InputState>,
                    _sub: gpui::Subscription,
                }
                let val: SharedString = app
                    .local_state
                    .read()
                    .unwrap()
                    .translation_keys
                    .get("chat.default_system_prompt")
                    .cloned()
                    .unwrap_or_default()
                    .into();
                let state =
                    window.use_keyed_state::<PromptState>("chat-sys-prompt", cx, |window, cx| {
                        let input = cx.new(|cx| {
                            InputState::new(window, cx)
                                .default_value(val)
                                .multi_line(true)
                        });
                        let app = app.clone();
                        let _sub = cx.subscribe(&input, {
                            move |_, emitter, event: &InputEvent, cx| {
                                if let InputEvent::Change = event {
                                    let v = emitter.read(cx).value();
                                    let mut state = app.local_state.write().unwrap();
                                    state
                                        .translation_keys
                                        .insert("chat.default_system_prompt".into(), v.to_string());
                                }
                            }
                        });
                        PromptState { input, _sub }
                    });
                let theme = cx.theme();
                let input = &state.read(cx).input;
                muted_input(Input::new(input), theme).into_any_element()
            })
        };

        let mut base = SettingPage::new(t(I18nKey::AiChatSettingsTab, l))
            .icon(Icon::new(IconName::BookOpen))
            .group(
                SettingGroup::new()
                    .title(t(I18nKey::DefaultSystemPrompt, l))
                    .item(sys_prompt),
            );

        if !ai_sel_opts.is_empty() {
            base = base.group(
                SettingGroup::new()
                    .title(if l == Language::ZhCn {
                        "后端选择"
                    } else {
                        "Backend Selection"
                    })
                    .item(SettingItem::render({
                        let ai_sel_opts = ai_sel_opts.clone();
                        let ai_sel_active = ai_sel_active.clone();
                        let app = app.clone();
                        move |_, _, cx| {
                            let l = lang(cx);
                            let current = ai_sel_active.clone();
                            let app_clone = app.clone();
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(div().text_sm().font_weight(gpui::FontWeight::BOLD).child(
                                    if l == Language::ZhCn {
                                        "当前后端"
                                    } else {
                                        "Active Backend"
                                    },
                                ))
                                .child(selector(
                                    "chat-ai-active-select",
                                    ai_sel_opts.clone(),
                                    current,
                                    false,
                                    move |v, _, _cx| {
                                        let mut state = app_clone.local_state.write().unwrap();
                                        state
                                            .translation_keys
                                            .insert("chat.active".into(), v.to_string());
                                        let _ = app_clone.local_state_manager.save_all(&state);
                                    },
                                ))
                        }
                    })),
            );
        }

        base
    }

    // ── Sync Page ──────────────────────────────────────────────────

    fn sync_page(&self, app: Arc<MainApp>, cx: &mut Context<Self>) -> SettingPage {
        let weak = cx.entity().downgrade();
        let l = lang(cx);

        // ── WebDAV group ───────────────────────────────────────────
        let webdav_group = {
            let app = app.clone();
            SettingGroup::new()
                .title(t(I18nKey::WebDavSettings, l))
                .item(SettingItem::new(
                    if l == Language::ZhCn { "启用 WebDAV" } else { "Enable WebDAV" },
                    SettingField::switch(
                        config_bool(|c| c.webdav.enabled),
                        set_config_bool(|c| &mut c.webdav.enabled),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::EndpointUrl, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.webdav.endpoint),
                        set_config_str(|c| &mut c.webdav.endpoint),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::Username, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.webdav.username),
                        set_config_str(|c| &mut c.webdav.username),
                    ),
                ))
                .item(SettingItem::render({
                    let app = app.clone();
                    move |_, window, cx| {
                        struct WdPassState {
                            input: Entity<InputState>,
                            _sub: gpui::Subscription,
                        }
                        let val: SharedString = app
                            .local_state
                            .read()
                            .unwrap()
                            .webdav_password
                            .clone()
                            .into();
                        let state = window.use_keyed_state::<WdPassState>(
                            "webdav-password",
                            cx,
                            |window, cx| {
                                let input = cx.new(|cx| {
                                    InputState::new(window, cx).default_value(val)
                                });
                                let app = app.clone();
                                let _sub = cx.subscribe(&input, {
                                    move |_, emitter, event: &InputEvent, cx| {
                                        if let InputEvent::Change = event {
                                            let v = emitter.read(cx).value();
                                            let mut s = app.local_state.write().unwrap();
                                            s.webdav_password = v.to_string();
                                        }
                                    }
                                });
                                WdPassState { input, _sub }
                            },
                        );
                        let theme = cx.theme();
                        let l = lang(cx);

                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .child(div().text_sm().child(t(I18nKey::Password, l))),
                            )
                            .child(
                                h_flex()
                                    .w_64()
                                    .child(
                                        muted_input(Input::new(&state.read(cx).input).mask_toggle(), theme)
                                            .flex_grow(1.0)
                                            .into_any_element()
                                    ),
                            )
                    }
                }))
                .item(SettingItem::new(
                    t(I18nKey::RemotePath, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.webdav.remote_path),
                        set_config_str(|c| &mut c.webdav.remote_path),
                    ),
                ))

                .item(SettingItem::render({
                    let app = app.clone();
                    let weak = weak.clone();
                    move |_, _window, cx| {
                        let l = lang(cx);
                        let theme = cx.theme();

                        let webdav_tested = weak.upgrade().map(|this| this.read(cx).webdav_tested).unwrap_or(false);
                        let webdav_test_result = weak.upgrade().and_then(|this| this.read(cx).webdav_test_result.clone());

                        h_flex()
                            .gap_4()
                            .justify_end()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .when_some(webdav_test_result, |this, res| match res {
                                        Ok(()) => this
                                            .child(Icon::new(IconName::Check).size(rems(0.875)).text_color(theme.success))
                                            .child(div().text_xs().text_color(theme.success).child(t(I18nKey::ConnectionSuccess, l))),
                                        Err(_) => this
                                            .child(Icon::new(IconName::TriangleAlert).size(rems(0.875)).text_color(theme.danger))
                                            .child(div().text_xs().text_color(theme.danger).child(t(I18nKey::ConnectionFailed, l))),
                                    })
                            )
                            .when(webdav_tested, |s| {
                                s.child(
                                    Button::new("sync-webdav-attachments")
                                        .label(t(I18nKey::SyncAttachments, l))
                                        .small()
                                        .primary()
                                        .on_click({
                                            let app = app.clone();
                                            let weak = weak.clone();
                                            move |_, _, cx| {
                                                let app = app.clone();
                                                cx.spawn(move |_: &mut AsyncApp| async move {
                                                    app.perform_attachments_sync();
                                                })
                                                .detach();
                                                if let Some(this) = weak.upgrade() {
                                                    this.update(cx, |_, cx| cx.notify());
                                                }
                                            }
                                        }),
                                )
                            })
                            .child(
                                Button::new("test-webdav")
                                    .label(t(I18nKey::TestConnection, l))
                                    .small()
                                    .on_click({
                                        let app = app.clone();
                                        let weak = weak.clone();
                                        move |_, window, cx| {
                                            let cfg = cx.global::<ConfigStore>().inner.clone();
                                            let app = app.clone();
                                            let weak = weak.clone();
                                            let handle = window.window_handle();
                                            let l = lang(cx);
                                            cx.spawn(move |cx: &mut AsyncApp| {
                                                let mut ax = cx.clone();
                                                async move {
                                                    let res = app
                                                        .test_webdav_config(
                                                            cfg.webdav.endpoint,
                                                            cfg.webdav.username,
                                                            app.local_state.read().unwrap().webdav_password.clone(),
                                                            cfg.webdav.remote_path,
                                                        )
                                                        .await;
                                                    let is_ok = res.is_ok();
                                                    let _ = ax.update_window(handle, |_, _, cx| {
                                                        if let Some(this) = weak.upgrade() {
                                                            this.update(cx, |this, cx| {
                                                                this.webdav_tested = is_ok;
                                                                if let Err(ref e) = res {
                                                                    crate::notification_bus::show_notification(crate::notification_bus::NotificationType::Error, format!("{}: {}", t(I18nKey::ConnectionFailed, l), e), cx);
                                                                }
                                                                this.webdav_test_result = Some(res);
                                                                cx.notify();
                                                            });
                                                        }
                                                    });
                                                }
                                            })
                                            .detach();
                                        }
                                    }),
                            )
                            .into_any_element()
                    }
                }))
        };

        // ── Google Drive group ─────────────────────────────────────
        let gdrive_group = {
            SettingGroup::new()
                .title(t(I18nKey::GoogleDriveSettings, l))
                .item(SettingItem::new(
                    if l == Language::ZhCn { "启用 Google Drive" } else { "Enable Google Drive" },
                    SettingField::switch(
                        config_bool(|c| c.google_drive.enabled),
                        set_config_bool(|c| &mut c.google_drive.enabled),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::ClientId, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.google_drive.client_id),
                        set_config_str(|c| &mut c.google_drive.client_id),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::ClientSecret, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.google_drive.client_secret),
                        set_config_str(|c| &mut c.google_drive.client_secret),
                    ),
                ))
                .item(SettingItem::render({
                    let app = app.clone();
                    let _weak = weak.clone();
                    move |_, _, cx| {
                        let cfg = cx.global::<ConfigStore>().inner.clone();
                        let theme = cx.theme();
                        let l = lang(cx);
                        let is_authorized = cfg.google_drive.authorized;
                        let status_color = if is_authorized {
                            theme.accent
                        } else {
                            theme.muted_foreground
                        };
                        let status_text = if is_authorized {
                            t(I18nKey::ConnectionSuccess, l)
                        } else {
                            t(I18nKey::ConnectionFailed, l)
                        };
                        let can_auth = cfg.google_drive.enabled
                            && !cfg.google_drive.client_id.is_empty()
                            && !cfg.google_drive.client_secret.is_empty();
                        h_flex()
                            .gap_2()
                            .child(
                                Label::new(status_text)
                                    .text_sm()
                                    .text_color(status_color),
                            )
                            .when(can_auth, |this| {
                                this.child(
                                        Button::new("authorize-google-drive")
                                        .label(t(I18nKey::Authorize, l))
                                        .small()
                                        .on_click({
                                            let app = app.clone();
                                            move |_, window, cx| {
                                                let cfg = cx
                                                    .global::<ConfigStore>()
                                                    .inner
                                                    .clone();
                                                let app = app.clone();
                                                let handle = window.window_handle();
                                                cx.spawn(move |cx: &mut AsyncApp| {
                                                    let mut ax = cx.clone();
                                                    async move {
                                                        let result = sync::google_drive::complete_oauth_flow(
                                                            &cfg.google_drive.client_id,
                                                            &cfg.google_drive.client_secret,
                                                        )
                                                        .await;
                                                        match result {
                                                            Ok(refresh_token) => {
                                                                 let mut state =
                                                                    app.local_state.write().unwrap();
                                                                 state.google_drive_refresh_token =
                                                                     refresh_token;
                                                                 let _ = ax.update_window(handle, |_, _, cx| {
                                                                     cx.set_global(ConfigStore {
                                                                         inner: app.config.lock().unwrap().clone(),
                                                                     });
                                                                 });
                                                            }
                                                            Err(e) => {
                                                                error!("Google Drive OAuth 失败: {e}");
                                                            }
                                                        }
                                                    }
                                                })
                                                .detach();
                                            }
                                        }),
                                )
                            })
                            .into_any_element()
                    }
                }))
        };

        // ── Database Sync group ────────────────────────────────────
        let db_group = {
            let app = app.clone();
            SettingGroup::new()
                .title(t(I18nKey::DatabaseSettings, l))
                .item(SettingItem::new(
                    if l == Language::ZhCn { "启用远程数据库" } else { "Use Remote Database" },
                    SettingField::switch(
                        config_bool(|c| c.database.use_remote),
                        set_config_bool(|c| &mut c.database.use_remote),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::Host, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.database.host),
                        set_config_str(|c| &mut c.database.host),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::Port, l),
                    SettingField::<SharedString>::input(
                        move |cx| cx.global::<ConfigStore>().database.port.to_string().into(),
                        move |v, cx| {
                            if let Ok(port) = v.parse::<u16>() {
                                cx.update_global::<ConfigStore, _>(|store, _| {
                                    store.inner.database.port = port;
                                });
                            }
                        },
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::DatabaseName, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.database.database),
                        set_config_str(|c| &mut c.database.database),
                    ),
                ))
                .item(SettingItem::new(
                    t(I18nKey::Username, l),
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.database.username),
                        set_config_str(|c| &mut c.database.username),
                    ),
                ))
                .item(SettingItem::render({
                    let app = app.clone();
                    move |_, window, cx| {
                        struct DbPassState {
                            input: Entity<InputState>,
                            _sub: gpui::Subscription,
                        }
                        let val: SharedString = config_str(|c| &c.database.password)(cx);
                        let state = window.use_keyed_state::<DbPassState>(
                            "db-password",
                            cx,
                            |window, cx| {
                                let input = cx.new(|cx| {
                                    InputState::new(window, cx).default_value(val)
                                });
                                let app = app.clone();
                                let _sub = cx.subscribe(&input, {
                                    move |_, emitter, event: &InputEvent, cx| {
                                        if let InputEvent::Change = event {
                                            let v = emitter.read(cx).value();
                                            cx.update_global::<ConfigStore, _>(|store, _| {
                                                store.inner.database.password = v.to_string();
                                            });
                                            let _ = app.update_config(cx.global::<ConfigStore>().inner.clone());
                                        }
                                    }
                                });
                                DbPassState { input, _sub }
                            },
                        );
                        let theme = cx.theme();
                        let l = lang(cx);

                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .child(div().text_sm().child(t(I18nKey::Password, l))),
                            )
                            .child(
                                h_flex()
                                    .w_64()
                                    .child(
                                        muted_input(Input::new(&state.read(cx).input).mask_toggle(), theme)
                                            .flex_grow(1.0)
                                            .into_any_element()
                                    ),
                            )
                    }
                }))
                .item(SettingItem::new(
                    t(I18nKey::EnableSSL, l),
                    SettingField::switch(
                        config_bool(|c| c.database.use_ssl),
                        set_config_bool(|c| &mut c.database.use_ssl),
                    ),
                ))
                .item(SettingItem::render({
                    let app = app.clone();
                    let weak = weak.clone();
                    move |_, _, cx| {
                        let l = lang(cx);
                        let theme = cx.theme();
                        let db_tested = weak.upgrade().map(|this| this.read(cx).db_tested).unwrap_or(false);
                        let db_test_result = weak.upgrade().and_then(|this| this.read(cx).db_test_result.clone());

                        h_flex()
                            .gap_4()
                            .justify_end()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .when_some(db_test_result, |this, res| match res {
                                        Ok(()) => this
                                            .child(Icon::new(IconName::Check).size(rems(0.875)).text_color(theme.success))
                                            .child(div().text_xs().text_color(theme.success).child(t(I18nKey::ConnectionSuccess, l))),
                                        Err(_) => this
                                            .child(Icon::new(IconName::TriangleAlert).size(rems(0.875)).text_color(theme.danger))
                                            .child(div().text_xs().text_color(theme.danger).child(t(I18nKey::ConnectionFailed, l))),
                                    })
                            )
                            .when(db_tested, |s| {
                                s.child(
                                    Button::new("sync-db-metadata")
                                        .label(t(I18nKey::SyncMetadata, l))
                                        .icon(IconName::Globe)
                                        .small()
                                        .primary()
                                        .on_click({
                                            let app = app.clone();
                                            let weak = weak.clone();
                                            move |_, _, cx| {
                                                let app = app.clone();
                                                cx.spawn(move |_: &mut AsyncApp| async move {
                                                    app.perform_sync();
                                                })
                                                .detach();
                                                if let Some(this) = weak.upgrade() {
                                                    this.update(cx, |_, cx| cx.notify());
                                                }
                                            }
                                        }),
                                )
                            })
                            .child(
                                Button::new("test-db")
                                    .label(t(I18nKey::TestConnection, l))
                                    .small()
                                    .on_click({
                                        let app = app.clone();
                                        let weak = weak.clone();
                                        move |_, window, cx| {
                                            let cfg = cx.global::<ConfigStore>().inner.database.clone();
                                            let app = app.clone();
                                            let weak = weak.clone();
                                            let handle = window.window_handle();
                                            let l = lang(cx);
                                            cx.spawn(move |cx: &mut AsyncApp| {
                                                let mut ax = cx.clone();
                                                async move {
                                                    let res = app.test_mysql_config(cfg).await;
                                                    let is_ok = res.is_ok();
                                                    let _ = ax.update_window(handle, |_, _, cx| {
                                                        if let Some(this) = weak.upgrade() {
                                                            this.update(cx, |this, cx| {
                                                                this.db_tested = is_ok;
                                                                if let Err(ref e) = res {
                                                                    crate::notification_bus::show_notification(crate::notification_bus::NotificationType::Error, format!("{}: {}", t(I18nKey::ConnectionFailed, l), e), cx);
                                                                }
                                                                this.db_test_result = Some(res);
                                                                cx.notify();
                                                            });
                                                        }
                                                    });
                                                }
                                            })
                                            .detach();
                                        }
                                    }),
                            )
                            .into_any_element()
                    }
                }))
        };

        // ── Data Management group ──────────────────────────────────
        let data_mgmt_group = {
            SettingGroup::new()
                .title(t(I18nKey::DataManagement, l))
                .item(SettingItem::render({
                    let app = app.clone();
                    move |_, _, cx| {
                        let l = lang(cx);
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("clear-local-db")
                                    .label(t(I18nKey::ClearLocalDb, l))
                                    .small()
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _, _cx| {
                                            if let Err(e) = app.clear_local_database() {
                                                error!("清空本地数据库失败: {e}");
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("clear-local-files")
                                    .label(t(I18nKey::ClearLocalFiles, l))
                                    .small()
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _, _cx| {
                                            if let Err(e) = app.file_manager.trash_all() {
                                                error!("清空本地文件失败: {e}");
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("clear-cloud-db")
                                    .label(t(I18nKey::ClearCloudDb, l))
                                    .small()
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _, cx| {
                                            let app = app.clone();
                                            cx.spawn(move |_: &mut AsyncApp| async move {
                                                if let Err(e) =
                                                    app.sync_service.clear_remote_database().await
                                                {
                                                    error!("清空云端数据库失败: {e}");
                                                }
                                            })
                                            .detach();
                                        }
                                    }),
                            )
                            .child(
                                Button::new("clear-cloud-files")
                                    .label(t(I18nKey::ClearCloudFiles, l))
                                    .small()
                                    .on_click({
                                        let app = app.clone();
                                        move |_, _, cx| {
                                            let app = app.clone();
                                            cx.spawn(move |_: &mut AsyncApp| async move {
                                                if let Err(e) =
                                                    app.sync_service.clear_remote_files().await
                                                {
                                                    error!("清空云端文件失败: {e}");
                                                }
                                            })
                                            .detach();
                                        }
                                    }),
                            )
                            .into_any_element()
                    }
                }))
        };

        SettingPage::new(t(I18nKey::Sync, l))
            .icon(Icon::new(IconName::Cloud))
            .group(webdav_group)
            .group(gdrive_group)
            .group(db_group)
            .group(data_mgmt_group)
    }

    // ── AI Backends Page ───────────────────────────────────────────

    fn ai_backends_page(&self, _app: Arc<MainApp>, cx: &mut Context<Self>) -> SettingPage {
        let weak = cx.entity().downgrade();
        let l = lang(cx);

        SettingPage::new(t(I18nKey::AiBackendsSettingsTab, l))
            .icon(Icon::new(IconName::Puzzle))
            .group(
                SettingGroup::new()
                    .title(t(I18nKey::AiBackendsSettingsTab, l))
                    .item(SettingItem::render({
                        let weak = weak.clone();
                        move |_, _window, cx| {
                            let theme = cx.theme();
                            let l = lang(cx);

                            let this = if let Some(t) = weak.upgrade() { t } else { return div().into_any_element(); };
                            let (
                                ai_entries,
                                ai_edit_target,
                                ai_adding_new,
                                ai_edit_enable_thinking,
                            ) = {
                                let t = this.read(cx);
                                (
                                    t.ai_entries.clone(),
                                    t.ai_edit_target,
                                    t.ai_adding_new,
                                    t.ai_edit_enable_thinking,
                                )
                            };
                            let (
                                ai_edit_name_input,
                                ai_edit_kind_value,
                                ai_edit_api_key_input,
                                ai_edit_api_base_input,
                                ai_edit_model_input,
                                ai_edit_context_window_input,
                                ai_edit_compression_strategy_value,
                            ) = {
                                let t = this.read(cx);
                                (
                                    t.ai_edit_name_input.clone(),
                                    t.ai_edit_kind_value.clone(),
                                    t.ai_edit_api_key_input.clone(),
                                    t.ai_edit_api_base_input.clone(),
                                    t.ai_edit_model_input.clone(),
                                    t.ai_edit_context_window_input.clone(),
                                    t.ai_edit_compression_strategy_value.clone(),
                                )
                            };

                            let mut list = v_flex().gap_2();

                            if !ai_adding_new && ai_edit_target.is_none() {
                                list = list.child(
                                    h_flex()
                                        .justify_between()
                                        .child(
                                            Label::new(t(I18nKey::AiBackendsSettingsTab, l))
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD),
                                        )
                                        .child(
                                            Button::new("add-ai-backend")
                                                .label(t(I18nKey::AiAddBackend, l))
                                                .icon(IconName::Plus)
                                                .primary()
                                                .on_click({
                                                    let weak = weak.clone();
                                                    move |_, window, cx| {
                                                        if let Some(this) = weak.upgrade() {
                                                            this.update(cx, |t, cx| {
                                                                t.ai_adding_new = true;
                                                                t.ai_edit_target = None;
                                                                t.ai_edit_name_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_api_key_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_api_base_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_model_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "", window, cx); });
                                                                t.ai_edit_context_window_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), "4096", window, cx); });
                                                                t.ai_edit_enable_thinking = false;
                                                                cx.notify();
                                                            });
                                                        }
                                                    }
                                                }),
                                        ),
                                );

                                for (i, entry) in ai_entries.iter().enumerate() {
                                    let kind_display = match entry.kind.as_str() {
                                        "openai" => "OpenAI",
                                        "ollama" => "Ollama",
                                        "claude" => "Claude",
                                        "siliconflow" => "SiliconFlow",
                                        _ => &entry.kind,
                                    };

                                    list = list.child(
                                        div()
                                            .relative()
                                            .rounded_md()
                                            .bg(theme.muted)
                                            .p(rems(0.75))
                                            .child(
                                                h_flex()
                                                    .absolute()
                                                    .top_2()
                                                    .right_2()
                                                    .gap_1()
                                                    .child(
                                                        Button::new(SharedString::from(format!("edit-backend-{}", i)))
                                                            .label(t(I18nKey::Edit, l))
                                                            .ghost()
                                                            .small()
                                                            .on_click({
                                                                let weak = weak.clone();
                                                                let entry = entry.clone();
                                                                move |_, window, cx| {
                                                                    if let Some(t) = weak.upgrade() {
                                                                        t.update(cx, |t, cx| {
                                                                            t.ai_edit_target = Some(i);
                                                                            t.ai_adding_new = false;
                                                                            t.ai_edit_name_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.name, window, cx); });
                                                                            t.ai_edit_kind_value = entry.kind.clone().into();
                                                                            t.ai_edit_api_key_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.api_key, window, cx); });
                                                                            t.ai_edit_api_base_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.api_base, window, cx); });
                                                                            t.ai_edit_model_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.model, window, cx); });
                                                                            t.ai_edit_context_window_input.update(cx, |s, cx| { let len = s.text().len(); s.replace_text_in_range(Some(0..len), &entry.context_window.to_string(), window, cx); });
                                                                            t.ai_edit_compression_strategy_value = entry.compression_strategy.clone().into();
                                                                            t.ai_edit_enable_thinking = entry.enable_thinking;
                                                                            cx.notify();
                                                                        });
                                                                    }
                                                                }
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new(SharedString::from(format!("delete-backend-{}", i)))
                                                            .icon(IconName::Trash)
                                                            .ghost()
                                                            .small()
                                                            .text_color(theme.danger)
                                                            .on_click({
                                                                let weak = weak.clone();
                                                                move |_, _, cx| {
                                                                    if let Some(t) = weak.upgrade() {
                                                                        t.update(cx, |t, cx| {
                                                                            t.ai_entries.remove(i);
                                                                            if t.ai_edit_target == Some(i) {
                                                                                t.ai_edit_target = None;
                                                                            } else if let Some(edit_i) = t.ai_edit_target {
                                                                                if edit_i > i {
                                                                                    t.ai_edit_target = Some(edit_i - 1);
                                                                                }
                                                                            }
                                                                            cx.notify();
                                                                        });
                                                                    }
                                                                }
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                v_flex()
                                                    .gap_1()
                                                    .child(Label::new(entry.name.clone()).text_sm().font_weight(gpui::FontWeight::BOLD))
                                                    .child(
                                                        Label::new(format!("{} • {} • {}", kind_display, entry.model, entry.api_base))
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground),
                                                    ),
                                            )
                                    );
                                }
                            } else {
                                // EDIT/ADD FORM
                                let title = if ai_adding_new {
                                    t(I18nKey::AiAddBackend, l).to_string()
                                } else {
                                    format!("{} {}", t(I18nKey::Edit, l), t(I18nKey::EngineAi, l))
                                };

                                list = list.child(
                                    v_flex()
                                        .gap_3()
                                        .child(Label::new(title).text_sm().font_weight(gpui::FontWeight::BOLD))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiBackendName, l)).w(rems(6.0))).child(muted_input(Input::new(&ai_edit_name_input), theme).flex_grow(1.0)))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiBackendType, l)).w(rems(6.0))).child({
                                            let weak = weak.clone();
                                            selector(
                                                "ai-edit-kind",
                                                vec![
                                                    ("openai".into(), "OpenAI".into()),
                                                    ("ollama".into(), "Ollama".into()),
                                                    ("claude".into(), "Claude".into()),
                                                    ("siliconflow".into(), "SiliconFlow".into()),
                                                ],
                                                ai_edit_kind_value.clone(),
                                                false,
                                                move |v, _, cx| {
                                                    if let Some(this) = weak.upgrade() {
                                                        this.update(cx, |this, _| {
                                                            this.ai_edit_kind_value = v;
                                                        });
                                                    }
                                                },
                                            )
                                        }))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiApiKey, l)).w(rems(6.0))).child(muted_input(Input::new(&ai_edit_api_key_input), theme).flex_grow(1.0)))
                                        .when(
                                            ai_edit_kind_value.as_ref() != "siliconflow",
                                            |this| this.child(h_flex().gap_2().child(Label::new(t(I18nKey::AiApiBase, l)).w(rems(6.0))).child(muted_input(Input::new(&ai_edit_api_base_input), theme).flex_grow(1.0)))
                                        )
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiModel, l)).w(rems(6.0))).child(muted_input(Input::new(&ai_edit_model_input), theme).flex_grow(1.0)))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiContextWindow, l)).w(rems(6.0))).child(muted_input(Input::new(&ai_edit_context_window_input), theme).flex_grow(1.0)))
                                        .child(h_flex().gap_2().child(Label::new(t(I18nKey::AiCompressionStrategy, l)).w(rems(6.0))).child({
                                            let weak = weak.clone();
                                            selector(
                                                "ai-edit-compression",
                                                vec![
                                                    ("none".into(), "None".into()),
                                                    ("summary".into(), "Summary".into()),
                                                ],
                                                ai_edit_compression_strategy_value.clone(),
                                                false,
                                                move |v, _, cx| {
                                                    if let Some(this) = weak.upgrade() {
                                                        this.update(cx, |this, _| {
                                                            this.ai_edit_compression_strategy_value = v;
                                                        });
                                                    }
                                                },
                                            )
                                        }))
                                        .child(h_flex().gap_2().child(Label::new(if l == Language::ZhCn { "启用思考过程" } else { "Thinking" }).w(rems(6.0))).child(Switch::new("enable-thinking").checked(ai_edit_enable_thinking).on_click({
                                            let weak = weak.clone();
                                            move |v: &bool, _, cx| {
                                                if let Some(this) = weak.upgrade() {
                                                    this.update(cx, |t, cx| { t.ai_edit_enable_thinking = *v; cx.notify(); });
                                                }
                                            }
                                        })))
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .justify_end()
                                                .child(
                                                    Button::new("cancel-edit-ai")
                                                        .label(t(I18nKey::Cancel, l))
                                                        .ghost()
                                                        .on_click({
                                                            let weak = weak.clone();
                                                            move |_, _, cx| {
                                                                if let Some(this) = weak.upgrade() {
                                                                    this.update(cx, |t, cx| {
                                                                        t.ai_adding_new = false;
                                                                        t.ai_edit_target = None;
                                                                        cx.notify();
                                                                    });
                                                                }
                                                            }
                                                        })
                                                )
                                                .child(
                                                    Button::new("save-edit-ai")
                                                        .label(t(I18nKey::Save, l))
                                                        .primary()
                                                        .on_click({
                                                            let weak = weak.clone();
                                                            move |_, _, cx| {
                                                                if let Some(this) = weak.upgrade() {
                                                                    this.update(cx, |t, cx| {
                                                                        let new_entry = translate::AiBackendEntry {
                                                                            name: t.ai_edit_name_input.read(cx).text().to_string(),
                                                                            kind: t.ai_edit_kind_value.to_string(),
                                                                            api_key: t.ai_edit_api_key_input.read(cx).text().to_string(),
                                                                            api_base: t.ai_edit_api_base_input.read(cx).text().to_string(),
                                                                            model: t.ai_edit_model_input.read(cx).text().to_string(),
                                                                            context_window: t.ai_edit_context_window_input.read(cx).text().to_string().parse().unwrap_or(4096),
                                                                            compression_strategy: t.ai_edit_compression_strategy_value.to_string(),
                                                                            enable_thinking: t.ai_edit_enable_thinking,
                                                                            max_tokens: 4096,
                                                                            temperature: 0.3,
                                                                        };

                                                                        if t.ai_adding_new {
                                                                            t.ai_entries.push(new_entry);
                                                                        } else if let Some(idx) = t.ai_edit_target {
                                                                            if idx < t.ai_entries.len() {
                                                                                t.ai_entries[idx] = new_entry;
                                                                            }
                                                                        }

                                                                        t.ai_adding_new = false;
                                                                        t.ai_edit_target = None;
                                                                        cx.notify();
                                                                    });
                                                                }
                                                            }
                                                        })
                                                )
                                        )
                                );
                            }

                            list.into_any_element()
                        }
                    })),
            )
    }
    // ── About ──────────────────────────────────────────────────────

    fn about_page(&self, cx: &mut Context<Self>) -> SettingPage {
        let l = lang(cx);
        SettingPage::new(t(I18nKey::About, l))
            .icon(Icon::new(IconName::Info))
            .group(
                SettingGroup::new().item(SettingItem::render(move |_, _, cx| {
                    let theme = cx.theme();
                    v_flex()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .size_full()
                        .py(rems(4.0))
                        .child(
                            div()
                                .size(rems(5.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(gpui::img("icons/app_icon.png").size(rems(6.0))),
                        )
                        .child(
                            Label::new("Lumen")
                                .text_2xl()
                                .font_weight(gpui::FontWeight::BOLD),
                        )
                        .child(
                            Label::new(env!("CARGO_PKG_VERSION"))
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            Label::new(t(I18nKey::AboutDesc, l))
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            Label::new(t(I18nKey::Copyright, l))
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        )
                        .child(
                            div()
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, |_, _, _| {
                                    open_url("https://github.com/LumenLib/Lumen");
                                })
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .items_center()
                                        .child(Icon::new(IconName::GitHub).size(rems(0.875)))
                                        .child(
                                            Label::new("GitHub")
                                                .text_sm()
                                                .text_color(theme.muted_foreground),
                                        ),
                                ),
                        )
                        .into_any_element()
                })),
            )
    }
}

impl gpui::Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let pages = self.pages(window, cx);

        let default_ix = self
            .initial_tab
            .map(|t| match t {
                SettingsTab::General => 0,
                SettingsTab::Sync => 1,
                SettingsTab::AiBackends => 2,
                SettingsTab::Translation => 3,
                SettingsTab::AiChat => 4,
                SettingsTab::About => 5,
            })
            .unwrap_or(0);

        let weak = cx.entity().downgrade();
        let sidebar_w = px(200.0);
        let _drag_state = window.use_state(cx, |_, _| false);
        let theme = cx.theme().clone();

        let settings = Settings::new("app-settings")
            .sidebar_width(sidebar_w)
            .default_selected_index(gpui_component::setting::SelectIndex {
                page_ix: default_ix,
                group_ix: None,
            })
            .pages(pages);

        let content: gpui::Stateful<gpui::Div> = {
            let base = div().size_full().child(settings);

            #[cfg(target_os = "macos")]
            {
                base.id("settings-content")
            }

            #[cfg(not(target_os = "macos"))]
            {
                let ds = _drag_state.clone();
                let sw = sidebar_w;
                base.id("settings-drag-area")
                    .on_mouse_down(MouseButton::Left, {
                        let ds = ds.clone();
                        move |event, _, cx| {
                            if event.position.y < px(40.0) && event.position.x >= sw {
                                ds.update(cx, |val, _| *val = true);
                                cx.stop_propagation();
                            }
                        }
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let ds = ds.clone();
                        move |_, _, cx| {
                            ds.update(cx, |val, _| *val = false);
                        }
                    })
                    .on_mouse_move({
                        let ds = ds.clone();
                        move |_, window, cx| {
                            if *ds.read(cx) {
                                ds.update(cx, |val, _| *val = false);
                                window.start_window_move();
                            }
                        }
                    })
            }
        };

        div()
            .v_flex()
            .size_full()
            .bg(theme.background)
            .when(cfg!(target_os = "macos"), |this| this.pt(px(32.0)))
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(content)
                    .child(
                        h_flex()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w(sidebar_w)
                            .px_3()
                            .py_2()
                            .gap_2()
                            .border_t_1()
                            .border_color(theme.border)
                            .bg(theme.background)
                            .child(
                                Button::new("cancel-settings")
                                    .ghost()
                                    .flex_1()
                                    .small()
                                    .icon(IconName::Close)
                                    .on_click({
                                        let weak = weak.clone();
                                        move |_, window, cx| {
                                            if let Some(mw) = weak.upgrade() {
                                                mw.update(cx, |this, cx| {
                                                    this.handle_cancel(window, cx)
                                                });
                                            }
                                        }
                                    }),
                            )
                            .child(
                                Button::new("save-settings")
                                    .primary()
                                    .flex_1()
                                    .small()
                                    .icon(IconName::Check)
                                    .on_click({
                                        let weak = weak.clone();
                                        move |_, window, cx| {
                                            if let Some(mw) = weak.upgrade() {
                                                mw.update(cx, |this, cx| {
                                                    this.handle_save(window, cx)
                                                });
                                            }
                                        }
                                    }),
                            ),
                    )
                    .child(self.toast_overlay.clone()),
            )
    }
}
