use crate::config::AppConfig;
use crate::config_store::ConfigStore;
use crate::services::MainApp;
use crate::ui::icons::IconName;
use crate::ui::theme_manager::LOADER;
use gpui::prelude::*;
use gpui::{
    App, AppContext, AsyncApp, Entity, EntityInputHandler, MouseButton, PathPromptOptions,
    SharedString, Window, WindowId, div, px, rems, transparent_black,
};
use gpui_component::{
    ActiveTheme, Icon, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    setting::{
        RenderOptions, SelectIndex, SettingField, SettingGroup, SettingItem, SettingPage, Settings,
    },
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
#[allow(dead_code)]
pub struct SettingsWindow {
    app: Arc<MainApp>,
    initial_config: AppConfig,
    saved_flag: Arc<std::sync::atomic::AtomicBool>,
    close_subscription: Option<gpui::Subscription>,
    toast_overlay: Entity<crate::ui::components::ToastOverlay>,
    initial_tab: Option<SettingsTab>,
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

fn setting_input(input: Input, theme: &gpui_component::Theme) -> gpui::Div {
    div()
        .bg(theme.muted)
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(input.appearance(false))
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
        .when(desc.len() > 0, |this| {
            this.child(
                Label::new(desc)
                    .text_xs()
                    .text_color(theme.muted_foreground),
            )
        })
        .child(
            h_flex()
                .gap_2()
                .child(setting_input(Input::new(&state.input), &theme).flex_grow(1.0))
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
                                    prompt: Some(prompt_str.clone().into()),
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
            move |cx: &mut App, _: WindowId| {
                cx.set_global(ConfigStore {
                    inner: initial_config.clone(),
                });
                if let Err(e) = app.update_config(initial_config.clone()) {
                    error!("恢复配置失败: {e}");
                }
            }
        }));

        Self {
            app,
            initial_config: config,
            saved_flag,
            close_subscription,
            toast_overlay: cx.new(|cx| crate::ui::components::ToastOverlay::new(window, cx)),
            initial_tab,
        }
    }

    fn handle_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("设置窗口: 开始保存配置");
        self.saved_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let new_config = cx.global::<ConfigStore>().inner.clone();

        // Persist translation keys + AI entries + password from local_state
        if let Ok(state) = self.app.local_state.read() {
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

    // ── Build all pages ────────────────────────────────────────────

    fn pages(&self, _window: &mut Window, cx: &mut Context<Self>) -> Vec<SettingPage> {
        let app = self.app.clone();
        let weak = cx.entity().downgrade();

        let save_cancel = {
            let weak = weak.clone();
            move |_: &RenderOptions, _: &mut Window, cx: &mut App| {
                let l = lang(cx);
                let theme = cx.theme();
                h_flex()
                    .gap_2()
                    .justify_end()
                    .pt_4()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(
                        Button::new("cancel-settings")
                            .label(t(I18nKey::Cancel, l))
                            .ghost()
                            .on_click({
                                let weak = weak.clone();
                                move |_, window, cx| {
                                    if let Some(mw) = weak.upgrade() {
                                        mw.update(cx, |this, cx| this.handle_cancel(window, cx));
                                    }
                                }
                            }),
                    )
                    .child(
                        Button::new("save-settings")
                            .label(t(I18nKey::Save, l))
                            .primary()
                            .on_click({
                                let weak = weak.clone();
                                move |_, window, cx| {
                                    if let Some(mw) = weak.upgrade() {
                                        mw.update(cx, |this, cx| this.handle_save(window, cx));
                                    }
                                }
                            }),
                    )
                    .into_any_element()
            }
        };

        vec![
            self.general_page(app.clone(), cx),
            self.sync_page(app.clone(), cx),
            self.ai_backends_page(app.clone(), cx),
            self.translation_page(app.clone(), cx),
            self.ai_chat_page(app.clone(), cx),
            self.about_page(cx)
                .group(SettingGroup::new().item(SettingItem::render(save_cancel))),
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

        // ── Theme mode pills ───────────────────────────────────────

        let app_for_theme = app.clone();
        let theme_mode_el = {
            SettingItem::render(move |_, _, cx| {
                let current = cx.global::<ConfigStore>().ui.theme_mode.clone();
                let theme = cx.theme();
                let l = lang(cx);

                let pill = |id: String, mode: String, label: String| {
                    let active = current == mode;
                    div()
                        .id(id.clone())
                        .px(rems(0.75))
                        .py(rems(0.375))
                        .rounded_md()
                        .cursor_pointer()
                        .bg(if active {
                            theme.sidebar_accent
                        } else {
                            transparent_black()
                        })
                        .hover(|this| if !active { this.bg(theme.muted) } else { this })
                        .on_mouse_down(MouseButton::Left, {
                            let mode = mode;
                            let app = app_for_theme.clone();
                            move |_, _, cx| {
                                cx.update_global::<ConfigStore, _>(|store, _| {
                                    store.inner.ui.theme_mode = mode.clone();
                                });
                                let _ = app.update_config(cx.global::<ConfigStore>().inner.clone());
                            }
                        })
                        .child(Label::new(label).text_sm())
                };
                let mk = |id: &str, mode: &str, label: &str| {
                    pill(id.to_string(), mode.to_string(), label.to_string())
                };

                h_flex()
                    .gap_1()
                    .child(mk("theme-light", "light", t(I18nKey::Light, l).as_ref()))
                    .child(mk("theme-dark", "dark", t(I18nKey::Dark, l).as_ref()))
                    .child(mk("theme-system", "system", t(I18nKey::System, l).as_ref()))
                    .into_any_element()
            })
        };

        // ── Library Settings group ─────────────────────────────────

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
                            t(I18nKey::DatabaseDir, l.clone()).into(),
                            t(I18nKey::DatabaseDir, l.clone()).as_ref(),
                            t(I18nKey::DatabaseDirDesc, l.clone()).as_ref(),
                            window,
                            cx,
                        )
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
                                t(I18nKey::PdfViewerPathMacos, lang(cx)).as_ref(),
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
                                t(I18nKey::PdfViewerPathWindows, lang(cx)).as_ref(),
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
                        .child(
                            setting_input(Input::new(&state.read(cx).input), &theme).flex_grow(1.0),
                        )
                        .into_any_element()
                }))
        };

        // ── Assemble General page ──────────────────────────────────

        SettingPage::new(t(I18nKey::General, l))
            .icon(Icon::new(IconName::Settings))
            .group(
                SettingGroup::new()
                    .title(t(I18nKey::GeneralOptions, l))
                    .item(SettingItem::new(
                        t(I18nKey::Language, l),
                        SettingField::<SharedString>::scrollable_dropdown(
                            lang_options,
                            config_str(|c| &c.ui.language),
                            set_config_str(|c| &mut c.ui.language),
                        ),
                    ))
                    .item(SettingItem::new(
                        t(I18nKey::ThemeStyle, l),
                        SettingField::<SharedString>::scrollable_dropdown(
                            theme_style_options,
                            config_str(|c| &c.ui.theme_style),
                            set_config_str(|c| &mut c.ui.theme_style),
                        ),
                    ))
                    .item(SettingItem::new(
                        t(I18nKey::UiScale, l),
                        SettingField::<SharedString>::scrollable_dropdown(
                            scale_options,
                            move |cx| {
                                format!("{:.1}", cx.global::<ConfigStore>().inner.ui.ui_scale)
                                    .into()
                            },
                            move |v, cx| {
                                let val: f32 = v.parse().unwrap_or(1.0);
                                cx.update_global::<ConfigStore, _>(|store, _| {
                                    store.inner.ui.ui_scale = val;
                                });
                                let _ = app.update_config(cx.global::<ConfigStore>().inner.clone());
                            },
                        ),
                    ))
                    .item(SettingItem::new(
                        t(I18nKey::LogLevel, l),
                        SettingField::<SharedString>::scrollable_dropdown(
                            log_options,
                            config_str(|c| &c.log_level),
                            set_config_str(|c| &mut c.log_level),
                        ),
                    ))
                    .item(SettingItem::new(
                        t(I18nKey::NotificationLevel, l),
                        SettingField::<SharedString>::scrollable_dropdown(
                            notif_options,
                            config_str(|c| &c.notification_level),
                            set_config_str(|c| &mut c.notification_level),
                        ),
                    ))
                    .item(theme_mode_el),
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

        let (ai_sel_opts, ai_sel_active): (Vec<(SharedString, SharedString)>, SharedString) = {
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

        // API key input helper
        let api_key_field =
            |key_id: &'static str, label: &'static str, key: &'static str| -> SettingItem {
                let app = app.clone();
                SettingItem::render(move |_, window, cx| {
                    struct KeyState {
                        input: Entity<InputState>,
                        _sub: gpui::Subscription,
                    }
                    let val: SharedString = app
                        .local_state
                        .read()
                        .unwrap()
                        .translation_keys
                        .get(key)
                        .cloned()
                        .unwrap_or_default()
                        .into();
                    let state = window.use_keyed_state::<KeyState>(
                        format!("tkey-{key_id}"),
                        cx,
                        |window, cx| {
                            let input = cx.new(|cx| InputState::new(window, cx).default_value(val));
                            let app = app.clone();
                            let k = key.to_string();
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
                    setting_input(Input::new(&state.read(cx).input), &theme)
                        .child(
                            Label::new(label)
                                .text_xs()
                                .text_color(theme.muted_foreground),
                        )
                        .into_any_element()
                })
            };

        let mut base = SettingPage::new(t(I18nKey::TranslationSettingsTab, l))
            .icon(Icon::new(IconName::Globe))
            .group(
                SettingGroup::new()
                    .title(t(I18nKey::TranslationSettings, l))
                    .item(SettingItem::new(
                        t(I18nKey::TranslationEngine, l),
                        SettingField::<SharedString>::scrollable_dropdown(
                            engines,
                            config_str(|c| &c.translation.engine),
                            set_config_str(|c| &mut c.translation.engine),
                        ),
                    ))
                    .item(SettingItem::new(
                        t(I18nKey::TargetLanguage, l),
                        SettingField::<SharedString>::input(
                            config_str(|c| &c.translation.target_language),
                            set_config_str(|c| &mut c.translation.target_language),
                        ),
                    ))
                    .item(api_key_field("google", "Google API Key", "google"))
                    .item(api_key_field("niutrans", "NiuTrans API Key", "niutrans"))
                    .item(api_key_field("baidu", "Baidu API Key", "baidu"))
                    .item(api_key_field("youdao", "Youdao API Key", "youdao"))
                    .item(api_key_field("deepl", "DeepL API Key", "deepl")),
            );

        if !ai_sel_opts.is_empty() {
            let app2 = app.clone();
            base = base.group(
                SettingGroup::new()
                    .title("AI Backend (for Translation)")
                    .item(SettingItem::new(
                        "Active Backend",
                        SettingField::<SharedString>::scrollable_dropdown(
                            ai_sel_opts,
                            move |_| ai_sel_active.clone(),
                            move |v, _| {
                                let mut state = app2.local_state.write().unwrap();
                                state
                                    .translation_keys
                                    .insert("ai.active".into(), v.to_string());
                            },
                        ),
                    )),
            );
        }

        base
    }

    // ── AI Chat Page ───────────────────────────────────────────────

    fn ai_chat_page(&self, app: Arc<MainApp>, _cx: &mut Context<Self>) -> SettingPage {
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
                setting_input(Input::new(input), &theme).into_any_element()
            })
        };

        let mut base = SettingPage::new("AI Chat")
            .icon(Icon::new(IconName::BookOpen))
            .group(SettingGroup::new().title("AI Chat").item(sys_prompt));

        if !ai_sel_opts.is_empty() {
            let app2 = app.clone();
            base = base.group(SettingGroup::new().title("Backend Selection").item(
                SettingItem::new(
                    "Active Backend",
                    SettingField::<SharedString>::scrollable_dropdown(
                        ai_sel_opts,
                        move |_| ai_sel_active.clone(),
                        move |v, _| {
                            let mut state = app2.local_state.write().unwrap();
                            state
                                .translation_keys
                                .insert("chat.active".into(), v.to_string());
                        },
                    ),
                ),
            ));
        }

        base
    }

    // ── Sync Page ──────────────────────────────────────────────────

    fn sync_page(&self, _app: Arc<MainApp>, cx: &mut Context<Self>) -> SettingPage {
        let _l = lang(cx);

        let webdav_group = {
            SettingGroup::new()
                .title("WebDAV")
                .item(SettingItem::new(
                    "Enable WebDAV",
                    SettingField::switch(
                        config_bool(|c| c.webdav.enabled),
                        set_config_bool(|c| &mut c.webdav.enabled),
                    ),
                ))
                .item(SettingItem::new(
                    "Endpoint",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.webdav.endpoint),
                        set_config_str(|c| &mut c.webdav.endpoint),
                    ),
                ))
                .item(SettingItem::new(
                    "Username",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.webdav.username),
                        set_config_str(|c| &mut c.webdav.username),
                    ),
                ))
                .item(SettingItem::new(
                    "Remote Path",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.webdav.remote_path),
                        set_config_str(|c| &mut c.webdav.remote_path),
                    ),
                ))
                .item(SettingItem::new(
                    "On-demand sync",
                    SettingField::switch(
                        config_bool(|c| c.webdav.on_demand),
                        set_config_bool(|c| &mut c.webdav.on_demand),
                    ),
                ))
        };

        let gdrive_group = {
            SettingGroup::new()
                .title("Google Drive")
                .item(SettingItem::new(
                    "Enable Google Drive",
                    SettingField::switch(
                        config_bool(|c| c.google_drive.enabled),
                        set_config_bool(|c| &mut c.google_drive.enabled),
                    ),
                ))
                .item(SettingItem::new(
                    "Client ID",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.google_drive.client_id),
                        set_config_str(|c| &mut c.google_drive.client_id),
                    ),
                ))
                .item(SettingItem::new(
                    "Client Secret",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.google_drive.client_secret),
                        set_config_str(|c| &mut c.google_drive.client_secret),
                    ),
                ))
        };

        let db_group = {
            SettingGroup::new()
                .title("Database Sync")
                .item(SettingItem::new(
                    "Use Remote",
                    SettingField::switch(
                        config_bool(|c| c.database.use_remote),
                        set_config_bool(|c| &mut c.database.use_remote),
                    ),
                ))
                .item(SettingItem::new(
                    "Host",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.database.host),
                        set_config_str(|c| &mut c.database.host),
                    ),
                ))
                .item(SettingItem::new(
                    "Port",
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
                    "Database",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.database.database),
                        set_config_str(|c| &mut c.database.database),
                    ),
                ))
                .item(SettingItem::new(
                    "Username",
                    SettingField::<SharedString>::input(
                        config_str(|c| &c.database.username),
                        set_config_str(|c| &mut c.database.username),
                    ),
                ))
                .item(SettingItem::new(
                    "Use SSL",
                    SettingField::switch(
                        config_bool(|c| c.database.use_ssl),
                        set_config_bool(|c| &mut c.database.use_ssl),
                    ),
                ))
        };

        SettingPage::new("Sync")
            .icon(Icon::new(IconName::Cloud))
            .group(webdav_group)
            .group(gdrive_group)
            .group(db_group)
    }

    // ── AI Backends Page ───────────────────────────────────────────

    fn ai_backends_page(&self, app: Arc<MainApp>, _cx: &mut Context<Self>) -> SettingPage {
        SettingPage::new("AI Backends")
            .icon(Icon::new(IconName::Puzzle))
            .group(
                SettingGroup::new()
                    .title("Backends")
                    .item(SettingItem::render({
                        let app = app.clone();
                        move |_, _, cx| {
                            let theme = cx.theme();
                            let keys = app.local_state.read().unwrap();
                            let entries_json = keys
                                .translation_keys
                                .get("ai.entries")
                                .cloned()
                                .unwrap_or_default();
                            drop(keys);
                            let entries: Vec<translate::AiBackendEntry> =
                                serde_json::from_str(&entries_json).unwrap_or_default();

                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .child(
                                            Label::new("Configured AI Backends")
                                                .text_sm()
                                                .font_weight(gpui::FontWeight::BOLD),
                                        )
                                        .child(
                                            Button::new("add-ai-backend")
                                                .label("+ Add")
                                                .icon(IconName::Plus)
                                                .primary(),
                                        ),
                                )
                                .children(entries.iter().map(|entry| {
                                    let kind_display = match entry.kind.as_str() {
                                        "openai" => "OpenAI",
                                        "ollama" => "Ollama",
                                        "claude" => "Claude",
                                        "siliconflow" => "SiliconFlow",
                                        _ => &entry.kind,
                                    };
                                    div().rounded_md().bg(theme.muted).p(rems(0.75)).child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                h_flex()
                                                    .justify_between()
                                                    .child(
                                                        Label::new(entry.name.clone())
                                                            .text_sm()
                                                            .font_weight(gpui::FontWeight::BOLD),
                                                    )
                                                    .child(
                                                        Label::new(kind_display)
                                                            .text_xs()
                                                            .text_color(theme.muted_foreground),
                                                    ),
                                            )
                                            .child(
                                                Label::new(format!("Model: {}", entry.model))
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground),
                                            )
                                            .child(
                                                Label::new(format!("Base URL: {}", entry.api_base))
                                                    .text_xs()
                                                    .text_color(theme.muted_foreground),
                                            ),
                                    )
                                }))
                                .into_any_element()
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
                            Label::new("v0.1.7")
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
                        .into_any_element()
                })),
            )
    }
}

impl gpui::Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let pages = self.pages(window, cx);
        let theme = cx.theme();

        let default_ix = self
            .initial_tab
            .and_then(|t| match t {
                SettingsTab::General => Some(0),
                SettingsTab::Sync => Some(1),
                SettingsTab::AiBackends => Some(2),
                SettingsTab::Translation => Some(3),
                SettingsTab::AiChat => Some(4),
                SettingsTab::About => Some(5),
            })
            .unwrap_or(0);

        div().v_flex().size_full().bg(theme.background).child(
            Settings::new("app-settings")
                .sidebar_width(px(200.0))
                .default_selected_index(SelectIndex {
                    page_ix: default_ix,
                    group_ix: None,
                })
                .pages(pages),
        )
    }
}
