mod about;
mod ai_backends;
mod ai_chat;
mod general;
mod sync;
mod translation;

use crate::app_state::config::ConfigStore;
use components::IconName;
use components::muted_input;
use gpui::prelude::*;
use gpui::{
    App, AppContext, AsyncApp, Entity, EntityInputHandler, PathPromptOptions, SharedString, Window,
    WindowId, div, px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState},
    label::Label,
    setting::{SettingItem, SettingPage, Settings},
    switch::Switch,
    v_flex,
};
use i18n::Language;
use log::{error, info};
use models::config::AppConfig;
use services::app::MainApp;
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

/// 带粗体标签的开关设置项（与通用选项页样式一致）
fn switch_setting_item(
    id: &'static str,
    label: impl Fn(Language) -> SharedString + Copy + 'static,
    get: impl Fn(&App) -> bool + Copy + 'static,
    set: impl Fn(bool, &mut App) + Copy + 'static,
) -> SettingItem {
    SettingItem::render(move |_, _, cx| {
        let l = lang(cx);
        let checked = get(cx);
        h_flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(label(l)),
            )
            .child(
                Switch::new(id)
                    .checked(checked)
                    .on_click(move |v: &bool, _, cx| set(*v, cx)),
            )
            .into_any_element()
    })
}

/// Render a path picker: input + browse button + async file dialog
#[allow(clippy::too_many_arguments)]
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
                .child(muted_input(&state.input, theme).flex_grow(1.0))
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
        let theme = cx.theme().clone();

        let settings = Settings::new("app-settings")
            .sidebar_width(sidebar_w)
            .default_selected_index(gpui_component::setting::SelectIndex {
                page_ix: default_ix,
                group_ix: None,
            })
            .pages(pages);

        let content = div().size_full().child(settings);

        // --- 沉浸式拖拽层（透明覆盖，不占布局空间）---
        let drag_overlay = components::add_drag_behavior(
            div()
                .id("settings-drag-overlay")
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(40.0)),
            window,
            cx,
        );

        div()
            .relative()
            .size_full()
            .bg(theme.background)
            .child(content)
            .child(drag_overlay)
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
                                        mw.update(cx, |this, cx| this.handle_cancel(window, cx));
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
                                        mw.update(cx, |this, cx| this.handle_save(window, cx));
                                    }
                                }
                            }),
                    ),
            )
            .child(self.toast_overlay.clone())
    }
}
