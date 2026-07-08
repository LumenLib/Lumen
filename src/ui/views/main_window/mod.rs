use crate::actions::EmptyTrash;
use crate::config_store::ConfigStore;
use crate::services::{
    AppViewMode, MainApp,
    data::{SortField, SortOrder},
    data_store::{DataStore, DataStoreEvent, RefreshMsg},
    ui_state::UiState,
};
use crate::ui::icons::IconName;
use crate::ui::{
    apply_theme,
    components::{FolderSelector, SettingsTab, TagSelector, ToastOverlay},
    views::{
        literature::{LiteratureDetailView, LiteratureListView, LiteraturePanel},
        subscription::{SubscriptionDetailView, SubscriptionListView, SubscriptionPanel},
        toolbar::{ToolbarEvent, ToolbarView},
    },
};
use gpui::prelude::*;
use gpui::{
    AppContext, AsyncApp, Entity, EventEmitter, KeyBinding, MouseButton, MouseMoveEvent, Pixels,
    Point, ReadGlobal, ScrollHandle, SharedString, Subscription, WeakEntity, Window, actions, div,
    px, rems,
};
use gpui_component::{ActiveTheme, Icon, h_flex, v_flex};
use i18n::{I18nKey, tf};
use models::Literature;
use pdf::PdfReaderView;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

mod actions;
mod layout;
mod menu;
mod modals;
pub(crate) mod utils;
pub use utils::render_separator;
mod types;

pub use menu::ContextMenuType;
pub(crate) use types::BatchSource;
pub use types::{FetchSource, TabId, ViewEvent};

actions!(main_window, [Cancel, ShowAbout, HandleSyncConflicts]);

impl EventEmitter<ViewEvent> for MainWindow {}
impl EventEmitter<ViewEvent> for FolderSelector {}

/// 主视图 - 整合三个面板
pub struct MainWindow {
    app: Arc<MainApp>,
    data_store: gpui::Entity<DataStore>,
    literature_panel: Entity<LiteraturePanel>,
    subscription_panel: Entity<SubscriptionPanel>,
    literature_list: Entity<LiteratureListView>,
    literature_detail: Entity<LiteratureDetailView>,
    subscription_list: Entity<SubscriptionListView>,
    subscription_detail: Entity<SubscriptionDetailView>,
    toolbar_view: Entity<ToolbarView>,
    left_width: Pixels,
    dragging_left: bool,
    dragging_right: bool,
    right_width: Pixels,
    current_window_width: Pixels,
    current_window_height: Pixels,
    /// 加载中模态框
    loading_modal: Option<String>,
    /// 全局右键菜单状态: (位置, 菜单视图)
    context_menu: Option<(Point<Pixels>, gpui::Entity<gpui_component::menu::PopupMenu>)>,
    /// 是否有活动的弹出窗口（设置、对比等）
    active_popup_count: u32,
    /// 当前激活的标签页
    active_tab: TabId,
    /// 已打开的 PDF 阅读器视图（doc_id → Option<Entity>，被卸载后为 None）
    open_pdf_tabs: HashMap<String, Option<Entity<PdfReaderView>>>,
    /// PDF 标签页的重新加载数据源（doc_id → (文献实体, 偏好路径)）
    pdf_tab_paths: HashMap<String, (Arc<Literature>, Option<PathBuf>)>,
    /// PDF 标签页的打开顺序（用于渲染 tab 顺序）
    open_pdf_tab_order: Vec<String>,
    /// PDF 标签页的活跃历史顺序（仅用于 LRU 内存淘汰）
    pdf_lru_order: Vec<String>,
    /// PDF 标签页的文献标题映射（doc_id → lit.title）
    pdf_tab_titles: HashMap<String, String>,
    /// PDF 标签栏横向滚动状态
    tab_scroll_handle: ScrollHandle,
    /// 标签选择器 (Entity, Position)
    tag_selector: Option<(Entity<TagSelector>, Point<Pixels>)>,
    /// 待处理的导入队列 (用于批量 BibTeX 导入)
    pending_imports: Vec<Literature>,
    /// 待处理的对比队列 (原始文献, 新文献)
    pending_compares: Vec<(Arc<Literature>, Literature)>,
    /// 待处理的选择器队列 (候选文献列表, 选择回调)
    pending_selectors: Vec<(
        Vec<Arc<Literature>>,
        Box<dyn Fn(&mut Self, Literature, &mut Window, &mut Context<Self>) + Send + Sync + 'static>,
    )>,
    /// 窗口边界变化订阅（保持引用以防止被丢弃）
    #[allow(dead_code)]
    pub bounds_subscription: Option<Subscription>,
    /// 窗口关闭事件订阅（保持引用以防止被丢弃）
    #[allow(dead_code)]
    pub close_subscription: Option<Subscription>,
    toast_overlay: Entity<ToastOverlay>,
}

impl MainWindow {
    pub fn new(
        app: Arc<MainApp>,
        data_store: gpui::Entity<DataStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // 同步初始主题
        let config = ConfigStore::global(cx).inner.clone();
        let (theme_mode, theme_style, ui_scale) = (
            config.ui.theme_mode.clone(),
            config.ui.theme_style.clone(),
            config.ui.ui_scale,
        );
        apply_theme(&theme_mode, &theme_style, ui_scale, cx);

        // UiState Global 已在 main.rs 中初始化，这里跳过
        let this_weak = cx.entity().downgrade();

        let literature_panel =
            cx.new(|_| LiteraturePanel::new(app.clone(), data_store.clone(), this_weak.clone()));

        let subscription_panel =
            cx.new(|_| SubscriptionPanel::new(app.clone(), data_store.clone(), this_weak.clone()));

        let literature_list =
            cx.new(|cx_inner| LiteratureListView::new(app.clone(), data_store.clone(), cx_inner));
        literature_list.update(cx, |this, cx| {
            this.register_actions(cx);
            this.set_parent_view(this_weak.clone());
        });

        // 绑定全局或局部快捷键
        let literature_detail =
            cx.new(|_| LiteratureDetailView::new(app.clone(), data_store.clone()));
        literature_detail.update(cx, |this, _| this.set_parent_view(this_weak.clone()));

        let subscription_list =
            cx.new(|cx_inner| SubscriptionListView::new(app.clone(), data_store.clone(), cx_inner));
        subscription_list.update(cx, |this, cx| {
            this.register_actions(cx);
            this.set_parent_view(this_weak.clone());
        });

        let subscription_detail =
            cx.new(|_cx| SubscriptionDetailView::new(app.clone(), data_store.clone()));

        let toolbar_view =
            cx.new(|cx| ToolbarView::new(app.clone(), data_store.clone(), window, cx));

        // 监听全局主题变化
        cx.observe_global::<gpui_component::Theme>(|_, cx| {
            cx.notify();
        })
        .detach();

        // 监听配置变更（主题/语言/缩放等）
        cx.observe_global::<ConfigStore>(|_, cx| {
            cx.notify();
        })
        .detach();

        // 订阅 DataStore 领域事件
        let data_store_entity = data_store.clone();
        cx.subscribe(
            &data_store_entity,
            |this, _entity: Entity<DataStore>, event: &DataStoreEvent, cx| match event {
                DataStoreEvent::DataChanged => {
                    this.literature_panel.update(cx, |_, cx| cx.notify());
                    this.subscription_panel.update(cx, |_, cx| cx.notify());
                    this.literature_list.update(cx, |panel, cx| {
                        panel.refresh_visible_literatures(cx);
                        cx.notify();
                    });
                    this.literature_detail.update(cx, |view, cx| {
                        view.reload_notes(cx);
                        cx.notify();
                    });
                    this.subscription_list.update(cx, |panel, cx| {
                        panel.refresh_visible_feed_items(cx);
                        cx.notify();
                    });
                    this.subscription_detail.update(cx, |_, cx| cx.notify());
                    // 通知所有处于激活/载入状态的 PDF 标签页重新加载笔记与会话
                    for view in this.open_pdf_tabs.values().flatten() {
                        view.update(cx, |v, cx| {
                            v.reload_notes(cx);
                            v.reload_chat_sessions(cx);
                        });
                    }
                }
            },
        )
        .detach();

        // 广播通道（桥接 MainApp 非 GPUI 上下文 → 所有窗口）
        let (tx, _) = tokio::sync::broadcast::channel::<RefreshMsg>(32);
        let mut rx = tx.subscribe();
        *app.refresh_tx.lock().unwrap() = Some(tx);
        let data_store_for_spawn = data_store.clone();
        let this_weak: gpui::WeakEntity<Self> = cx.entity().downgrade();
        cx.spawn(move |_: gpui::WeakEntity<Self>, cx: &mut gpui::AsyncApp| {
            let mut cx = cx.clone();
            let this_weak = this_weak.clone();
            async move {
                loop {
                    match rx.recv().await {
                        Ok(RefreshMsg::DataChanged) => {
                            let _ = cx.update(|cx| {
                                data_store_for_spawn.update(cx, |store, cx| {
                                    if let Err(e) = store.refresh_from_db(cx) {
                                        log::error!("DataStore: bridge refresh_from_db 失败: {e}");
                                    }
                                });
                            });
                        }
                        Ok(RefreshMsg::UiChanged) => {
                            let _ = this_weak.update(&mut cx, |_, cx| cx.notify());
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            log::warn!("RefreshMsg 通道滞后 {n} 条消息");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        })
        .detach();

        // 注册全局快捷键
        cx.bind_keys([KeyBinding::new("escape", Cancel, None)]);

        let (saved_left, saved_right) = if let Ok(state) = app.local_state.read() {
            (state.left_sidebar_width, state.right_sidebar_width)
        } else {
            (None, None)
        };

        let mut main_window = Self {
            app,
            data_store,
            literature_panel,
            subscription_panel,
            literature_list,
            literature_detail,
            subscription_list,
            subscription_detail,
            toolbar_view: toolbar_view.clone(),
            left_width: saved_left.map_or(window.rem_size() * 15.0, |v| {
                px((v as f32).clamp(150.0, 450.0))
            }),
            dragging_left: false,
            dragging_right: false,
            right_width: saved_right.map_or(window.rem_size() * 15.0, |v| {
                px((v as f32).clamp(150.0, 450.0))
            }),
            current_window_width: window.rem_size() * 75.0,
            current_window_height: window.rem_size() * 50.0,
            loading_modal: None,
            context_menu: None,
            active_popup_count: 0,
            active_tab: TabId::Main,
            open_pdf_tabs: HashMap::new(),
            pdf_tab_paths: HashMap::new(),
            open_pdf_tab_order: Vec::new(),
            pdf_lru_order: Vec::new(),
            pdf_tab_titles: HashMap::new(),
            tab_scroll_handle: ScrollHandle::new(),
            tag_selector: None,
            pending_imports: Vec::new(),
            pending_compares: Vec::new(),
            pending_selectors: Vec::new(),
            bounds_subscription: None,
            close_subscription: None,
            toast_overlay: cx.new(|cx| ToastOverlay::new(window, cx)),
        };

        // 处理工具栏事件
        main_window.handle_toolbar_events(&toolbar_view, window, cx);

        main_window
    }

    /// 处理工具栏发出的事件
    fn handle_toolbar_events(
        &mut self,
        toolbar_view: &Entity<ToolbarView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let this_weak = cx.entity().downgrade();
        let window_handle = window.window_handle();

        cx.subscribe(toolbar_view, move |_, _, event, cx| {
            let event = event.clone();
            let this_weak = this_weak.clone();
            cx.spawn(move |_, cx: &mut AsyncApp| {
                let mut cx = cx.clone();
                let event = event.clone();
                let this_weak = this_weak.clone();
                async move {
                    match event {
                        ToolbarEvent::Search(query) => {
                            let _ = this_weak.update(&mut cx, |this, cx| {
                                this.literature_list.update(cx, |list, cx| {
                                    list.set_search_text(query, cx);
                                });
                            });
                        }
                        ToolbarEvent::OpenManualAdd => {
                            let _ = cx.update_window(window_handle, |_, _window, cx| {
                                if let Some(this) = this_weak.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.open_manual_add_modal(cx);
                                    });
                                }
                            });
                        }
                        ToolbarEvent::OpenFetch(mode) => {
                            let _ = cx.update_window(window_handle, |_, _window, cx| {
                                if let Some(this) = this_weak.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.open_fetch_modal(mode, cx);
                                    });
                                }
                            });
                        }
                        ToolbarEvent::RunDuplicateDetection => {
                            let _ = cx.update_window(window_handle, |_, _window, cx| {
                                if let Some(this) = this_weak.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.run_duplicate_detection(cx);
                                    });
                                }
                            });
                        }
                        ToolbarEvent::AddSubscriptionToLibrary => {
                            let res = this_weak.update(&mut cx, |this, cx| {
                                let app = this.app.clone();
                                let selected_ids: Vec<String> = {
                                    let state = cx.global::<UiState>();
                                    state.selected_feed_item_ids.iter().cloned().collect()
                                };
                                (app, selected_ids)
                            });
                            if let Ok((app, selected_ids)) = res {
                                cx.background_executor()
                                    .spawn(async move {
                                        for id in selected_ids {
                                            let _ = app.add_feed_item_to_library(&id);
                                        }
                                    })
                                    .detach();
                            }
                        }
                        ToolbarEvent::AddSubscriptionToFolder(folder_id) => {
                            let res = this_weak.update(&mut cx, |this, cx| {
                                let app = this.app.clone();
                                let selected_ids: Vec<String> = {
                                    let state = cx.global::<UiState>();
                                    state.selected_feed_item_ids.iter().cloned().collect()
                                };
                                (app, selected_ids, folder_id.clone())
                            });
                            if let Ok((app, selected_ids, folder_id)) = res {
                                cx.background_executor()
                                    .spawn(async move {
                                        for id in selected_ids {
                                            // 先添加到文献库，获取新创建的文献ID
                                            if let Ok(lit_id) = app.add_feed_item_to_library(&id) {
                                                // 如果指定了文件夹，再添加到文件夹
                                                if let Some(ref fid) = folder_id {
                                                    let _ =
                                                        app.add_literature_to_folder(&lit_id, fid);
                                                }
                                            }
                                        }
                                    })
                                    .detach();
                            }
                        }
                        ToolbarEvent::ShowFolderSelector(pos) => {
                            let _ = this_weak.update(&mut cx, |this, cx| {
                                let folders = this.data_store.read(cx).folders.clone();

                                let app = this.app.clone();
                                let toolbar_weak = this.toolbar_view.downgrade();

                                let folder_selector = cx.new(|_| {
                                    FolderSelector::new(
                                        app.clone(),
                                        folders,
                                        true,
                                        move |folder_id: Option<String>, _, inner_cx| {
                                            inner_cx.emit(ViewEvent::CloseMenu);
                                            // 触发添加到文件夹事件
                                            if let Some(toolbar) = toolbar_weak.upgrade() {
                                                toolbar.update(inner_cx, |_, inner_cx| {
                                                    inner_cx.emit(
                                                        ToolbarEvent::AddSubscriptionToFolder(
                                                            folder_id,
                                                        ),
                                                    );
                                                });
                                            }
                                        },
                                    )
                                });

                                // 订阅FolderSelector的事件
                                cx.subscribe(&folder_selector, |this, _, event, cx| match event {
                                    ViewEvent::CloseMenu => {
                                        this.toolbar_view.update(cx, |toolbar, cx| {
                                            toolbar.folder_selector = None;
                                            cx.notify();
                                        });
                                    }
                                })
                                .detach();

                                this.toolbar_view.update(cx, |toolbar, cx| {
                                    toolbar.folder_selector = Some((folder_selector, pos));
                                    cx.notify();
                                });
                            });
                        }
                        ToolbarEvent::SortChanged(field, order) => {
                            let _ = this_weak.update(&mut cx, |this, cx| {
                                UiState::update(cx, |state| {
                                    state.sort_field = field;
                                    state.sort_order = order;
                                });

                                if let Ok(mut state) = this.app.local_state.write() {
                                    state.sort_field = Some(match field {
                                        SortField::Title => "Title".to_string(),
                                        SortField::Author => "Author".to_string(),
                                        SortField::Year => "Year".to_string(),
                                        SortField::Journal => "Journal".to_string(),
                                    });
                                    state.sort_asc = matches!(order, SortOrder::Ascending);
                                }

                                this.app.notify_ui_changed();

                                this.literature_list.update(cx, |list, cx| {
                                    list.refresh_visible_literatures(cx);
                                });
                            });
                        }
                    }
                }
            })
            .detach();
        })
        .detach();
    }

    pub fn handle_batch_fetch_metadata(
        &mut self,
        lit_ids: Vec<String>,
        source_type: crate::ui::views::main_window::types::BatchSource,
        cx: &mut Context<Self>,
    ) {
        if lit_ids.is_empty() {
            return;
        }

        let app = self.app.clone();
        let data_store = self.data_store.clone();
        let total = lit_ids.len();
        let lang = self.app.current_language();
        self.loading_modal = Some(tf(
            I18nKey::BatchUpdatingMetadata,
            lang,
            &["0", &total.to_string()],
        ));
        cx.notify();

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx_inner = cx.clone();
            async move {
                let mut results = Vec::new();
                {
                    for (i, id) in lit_ids.into_iter().enumerate() {
                        // 更新进度提示
                        let _ = this.update(&mut cx_inner, |this, cx: &mut Context<Self>| {
                            this.loading_modal = Some(tf(
                                I18nKey::BatchUpdatingMetadata,
                                lang,
                                &[&(i + 1).to_string(), &total.to_string()],
                            ));
                            cx.notify();
                        });

                        let lit_opt = cx_inner
                            .update(|cx| {
                                data_store
                                    .read(cx)
                                    .literatures
                                    .iter()
                                    .find(|l| l.id == id)
                                    .cloned()
                            })
                            .ok()
                            .flatten();

                        if let Some(lit) = lit_opt {
                            let source = match source_type {
                                BatchSource::ArXiv => {
                                    crate::ui::views::main_window::utils::extract_arxiv_id(&lit)
                                        .map(FetchSource::ArXiv)
                                }
                                BatchSource::Doi => lit
                                    .doi
                                    .as_ref()
                                    .filter(|d| !d.trim().is_empty())
                                    .map(|d| FetchSource::Doi(d.clone())),
                                BatchSource::Dblp => {
                                    if lit.title.is_empty() {
                                        None
                                    } else {
                                        Some(FetchSource::Dblp(lit.title.clone()))
                                    }
                                }
                                BatchSource::OpenAlex => {
                                    if let Some(doi) = &lit.doi {
                                        Some(FetchSource::OpenAlexDoi(doi.clone()))
                                    } else if !lit.title.is_empty() {
                                        Some(FetchSource::OpenAlexTitle(lit.title.clone()))
                                    } else {
                                        None
                                    }
                                }
                            };

                            if let Some(source) = source {
                                // 修复闪退：在 Tokio 运行时中执行网络请求
                                let app_clone = app.clone();
                                let handle = crate::RUNTIME.spawn(async move {
                                    app_clone.fetch_metadata_from_source(source).await
                                });

                                match handle.await {
                                    Ok(Ok(remote_lit)) => {
                                        results.push((lit, remote_lit));
                                    }
                                    Ok(Err(e)) => {
                                        log::error!("Batch fetch failed for {id}: {e}");
                                    }
                                    Err(e) => {
                                        log::error!("Tokio task join failed for {id}: {e}");
                                    }
                                }
                            }
                        }
                    }
                }

                let _ = this.update(&mut cx_inner, |this, cx: &mut Context<Self>| {
                    this.loading_modal = None;
                    this.pending_compares.extend(results);
                    this.process_next_batch_compare(cx);
                    this.app.notify_data_changed();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// 处理对比队列中的下一项
    pub fn process_next_batch_compare(&mut self, cx: &mut Context<Self>) {
        if self.pending_compares.is_empty() {
            return;
        }

        let (original, remote) = self.pending_compares.remove(0);
        self.show_literature_compare_with_callback(original, remote, cx, |this, cx| {
            this.process_next_batch_compare(cx);
        });
    }

    // =========================================================================
    // UI 状态变更方法（写入 UiState Global + LocalState）
    // =========================================================================
    pub fn select_folder(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_folder_id = Some(id.clone());
            state.selected_tag_id = None;
            state.selected_literature_ids.clear();
        });
        if let Ok(mut state) = self.app.local_state.write() {
            state.selected_sidebar_item = Some(format!("folder:{id}"));
        }
        self.literature_list.update(cx, |list, cx| {
            list.refresh_visible_literatures(cx);
        });
    }
    pub fn select_tag(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_tag_id = Some(id.clone());
            state.selected_folder_id = None;
            state.selected_literature_ids.clear();
        });
        if let Ok(mut state) = self.app.local_state.write() {
            state.selected_sidebar_item = Some(format!("tag:{id}"));
        }
        self.literature_list.update(cx, |list, cx| {
            list.refresh_visible_literatures(cx);
        });
    }
    pub fn select_literature(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_literature_ids.clear();
            state.selected_literature_ids.insert(id);
        });
    }
    pub fn toggle_literature_selection(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            if state.selected_literature_ids.contains(&id) {
                state.selected_literature_ids.remove(&id);
            } else {
                state.selected_literature_ids.insert(id);
            }
        });
    }
    pub fn add_literature_selection(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_literature_ids.insert(id);
        });
    }
    pub fn select_feed(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_feed_id = Some(id);
            state.selected_feed_item_ids.clear();
        });
    }
    pub fn select_feed_item(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_feed_item_ids.clear();
            state.selected_feed_item_ids.insert(id);
        });
    }
    pub fn toggle_feed_item_selection(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            if state.selected_feed_item_ids.contains(&id) {
                state.selected_feed_item_ids.remove(&id);
            } else {
                state.selected_feed_item_ids.insert(id);
            }
        });
    }
    pub fn add_feed_item_selection(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.selected_feed_item_ids.insert(id);
        });
    }
    pub fn set_view_mode(&mut self, mode: AppViewMode, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            state.view_mode = mode;
        });
    }
}

impl MainWindow {
    fn render_tab_bar(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let is_main_active = self.active_tab == TabId::Main;

        h_flex()
            .h(rems(1.75))
            .w_full()
            .flex_shrink_0()
            .bg(theme.background)
            .border_b_1()
            .border_color(theme.border)
            .items_center()
            .gap_1()
            .px_2()
            // macOS：左侧留空给系统交通灯
            .when(cfg!(target_os = "macos"), |this| {
                this.child(
                    div()
                        .w(rems(4.0))
                        .h_full()
                        .window_control_area(gpui::WindowControlArea::Drag),
                )
            })
            // 主页标签
            .child(
                div()
                    .id("tab-main")
                    .px(rems(1.125))
                    .py(rems(0.3))
                    .rounded_sm()
                    .cursor_pointer()
                    .when(is_main_active, |this| this.bg(theme.accent))
                    .when(!is_main_active, |this| {
                        this.hover(|this| this.bg(theme.muted))
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.active_tab = TabId::Main;
                            cx.notify();
                        }),
                    )
                    .child(Icon::new(IconName::Home).size(rems(1.0)).text_color(
                        if is_main_active {
                            theme.accent_foreground
                        } else {
                            theme.foreground
                        },
                    )),
            )
            // PDF 标签（可滚动区域）
            .child(
                div()
                    .id("tab-scroll-area")
                    .flex()
                    .flex_row()
                    .flex_grow()
                    .min_w(px(0.0))
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_scroll_handle)
                    .items_center()
                    .gap_1()
                    .children(self.open_pdf_tab_order.iter().map(|doc_id| {
                        let is_active = matches!(&self.active_tab, TabId::Pdf(id) if id == doc_id);
                        let title = self
                            .pdf_tab_titles
                            .get(doc_id)
                            .map(|s| s.as_str())
                            .unwrap_or(doc_id);
                        let tab_id: SharedString = format!("tab-pdf-{doc_id}").into();
                        let doc_id_for_click = doc_id.clone();
                        let doc_id_for_close = doc_id.clone();

                        div()
                            .id(tab_id)
                            .px(rems(0.75))
                            .py(rems(0.3))
                            .rounded_sm()
                            .cursor_pointer()
                            .when(is_active, |this| {
                                this.bg(theme.accent).text_color(theme.accent_foreground)
                            })
                            .when(!is_active, |this| {
                                this.hover(|this| this.bg(theme.muted))
                                    .text_color(theme.foreground)
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.activate_pdf_tab(doc_id_for_click.clone(), cx);
                                }),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .items_center()
                                    .child(
                                        div()
                                            .max_w(rems(12.0))
                                            .truncate()
                                            .text_size(rems(0.75))
                                            .child(title.to_string()),
                                    )
                                    .child(
                                        div()
                                            .cursor_pointer()
                                            .rounded_sm()
                                            .hover(|this| this.bg(gpui::red().opacity(0.3)))
                                            .px(rems(0.25))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.close_pdf_tab(&doc_id_for_close, cx);
                                                }),
                                            )
                                            .text_size(rems(0.75))
                                            .child("✕"),
                                    ),
                            )
                            .into_any_element()
                    })),
            )
            // 弹性区（拖拽窗口）
            .child(
                div()
                    .flex_grow()
                    .h_full()
                    .window_control_area(gpui::WindowControlArea::Drag),
            )
            // 设置按钮
            .child(
                div()
                    .id("tab-settings")
                    .px(rems(0.75))
                    .py(rems(0.3))
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|this| this.bg(theme.muted))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.open_settings_modal(cx, None);
                        }),
                    )
                    .child(
                        Icon::new(IconName::Settings)
                            .size(rems(1.0))
                            .text_color(theme.foreground),
                    ),
            )
            // 窗口控件（Windows/Linux 紧靠右侧）
            .when(cfg!(not(target_os = "macos")), |this| {
                let c = theme.foreground;
                let btn_w = px(36.0);
                let btn_h = px(30.0);
                let is_maximized = window.is_maximized();

                this.child(
                    h_flex()
                        .h_full()
                        .items_center()
                        .mr(-px(2.0))
                        .child(
                            div()
                                .id("window-minimize")
                                .w(btn_w)
                                .h(btn_h)
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .occlude()
                                .window_control_area(gpui::WindowControlArea::Min)
                                .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                .on_mouse_down(MouseButton::Left, |_, w: &mut Window, _cx| {
                                    w.minimize_window()
                                })
                                .child(
                                    Icon::new(IconName::Minimize).size(rems(0.85)).text_color(c),
                                ),
                        )
                        .child(
                            div()
                                .id("window-maximize-restore")
                                .w(btn_w)
                                .h(btn_h)
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .occlude()
                                .window_control_area(gpui::WindowControlArea::Max)
                                .hover(|s| s.bg(theme.muted.opacity(0.6)))
                                .on_mouse_down(MouseButton::Left, |_, w: &mut Window, _cx| {
                                    w.zoom_window()
                                })
                                .child(
                                    Icon::new(if is_maximized {
                                        IconName::Restore
                                    } else {
                                        IconName::Maximize
                                    })
                                    .size(rems(0.85))
                                    .text_color(c),
                                ),
                        )
                        .child(
                            div()
                                .id("window-close")
                                .w(btn_w)
                                .h(btn_h)
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .occlude()
                                .window_control_area(gpui::WindowControlArea::Close)
                                .hover(|s| s.bg(gpui::red().opacity(0.85)))
                                .on_mouse_down(MouseButton::Left, |_, win: &mut Window, _cx| {
                                    win.remove_window()
                                })
                                .child(Icon::new(IconName::Close).size(rems(0.85)).text_color(c)),
                        ),
                )
            })
    }

    fn close_pdf_tab(&mut self, doc_id: &str, cx: &mut Context<Self>) {
        self.open_pdf_tabs.remove(doc_id);
        self.pdf_tab_titles.remove(doc_id);
        self.pdf_tab_paths.remove(doc_id);
        self.open_pdf_tab_order.retain(|id| id != doc_id);
        self.pdf_lru_order.retain(|id| id != doc_id);
        if matches!(&self.active_tab, TabId::Pdf(id) if id == doc_id) {
            self.active_tab = TabId::Main;
        }
        cx.notify();
    }

    /// 将标签栏滚动到当前活跃标签可见位置
    fn scroll_to_active_tab(&self) {
        if let TabId::Pdf(id) = &self.active_tab {
            if let Some(idx) = self.open_pdf_tab_order.iter().position(|d| d == id) {
                // 估算每个标签宽度：padding(0.75*2) + 标题(~2rem) + 关闭按钮(~0.5rem) + gap(0.25)
                let tab_width_rems = 3.75;
                let offset_rems = (idx as f32 * tab_width_rems).max(0.0);
                // 使用 16px 基准字体大小（近似值）
                self.tab_scroll_handle
                    .set_offset(Point::new(px(offset_rems * 16.0), px(0.0)));
            }
        }
    }

    /// 激活指定的 PDF 标签页。如果它当前处于卸载状态 (None)，则触发重新实例化。
    pub fn activate_pdf_tab(&mut self, doc_id: String, cx: &mut Context<Self>) {
        self.active_tab = TabId::Pdf(doc_id.clone());
        self.scroll_to_active_tab();

        // 维护独立的 LRU 活跃历史顺序（仅用于后台内存卸载，绝不重新排列视觉上的 open_pdf_tab_order）
        self.pdf_lru_order.retain(|id| id != &doc_id);
        self.pdf_lru_order.push(doc_id.clone());

        // 如果对应的视图已从内存中卸载 (即值为 None)，则重新实例化并加载
        if self.open_pdf_tabs.get(&doc_id).is_none_or(|v| v.is_none()) {
            self.reload_pdf_tab(doc_id.clone(), cx);
        }
        cx.notify();
    }

    /// 重新加载或首次加载指定的 PDF 阅读器实例
    fn reload_pdf_tab(&mut self, doc_id: String, cx: &mut Context<Self>) {
        if let Some((lit, preferred_path)) = self.pdf_tab_paths.get(&doc_id).cloned() {
            let path = preferred_path.clone().or_else(|| {
                lit.attachments
                    .iter()
                    .find(|a| a.is_main)
                    .map(|a| PathBuf::from(&a.file_path))
            });
            let Some(path) = path else {
                return;
            };

            let app = self.app.clone();
            let doc_id_for_open = doc_id.clone();
            let lit_id = lit.id.clone();

            let (pdf_service, response_rx) =
                pdf::PdfService::new(path.clone()).expect("Failed to create PdfService");
            let delegate = Arc::new(crate::ui::views::main_window::actions::AppPdfDelegate {
                app: app.clone(),
                literature_id: lit_id,
            });

            let view = cx.new(|cx| {
                let mut view = PdfReaderView::new(pdf_service, Some(delegate), doc_id_for_open, cx);
                view.set_tab_bar_offset_rems(1.75);
                view.set_document_title(lit.title.clone());
                view.init_workers(response_rx, cx);
                view
            });

            // 监听语言配置变化
            cx.observe_global::<ConfigStore>({
                let view_weak = view.downgrade();
                move |_this: &mut Self, cx: &mut gpui::Context<Self>| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            let lang = cx.global::<ConfigStore>().current_language();
                            this.set_language(lang, cx);
                        });
                    }
                }
            })
            .detach();

            // 存入加载完毕的实例
            self.open_pdf_tabs.insert(doc_id.clone(), Some(view));

            // 控制活跃的实例数量不超过 3 个
            self.evict_stale_pdf_tabs(doc_id, cx);
        }
    }

    /// 淘汰最旧的、非当前活跃的 PDF 实例以节约内存
    fn evict_stale_pdf_tabs(&mut self, active_doc_id: String, cx: &mut Context<Self>) {
        let active_instances: Vec<String> = self
            .open_pdf_tabs
            .iter()
            .filter(|(_, opt)| opt.is_some())
            .map(|(id, _)| id.clone())
            .collect();

        if active_instances.len() > 3 {
            // 从 pdf_lru_order 活跃历史中，自前向后寻找最旧的一个、非当前激活的、且已在内存中载入的项进行卸载
            let oldest_stale_id = self
                .pdf_lru_order
                .iter()
                .find(|&id| id != &active_doc_id && active_instances.contains(id))
                .cloned();

            if let Some(stale_id) = oldest_stale_id {
                log::info!(
                    "MainWindow: 内存活跃 PDF 达到上限(3)，卸载最旧的 PDF 实例以释放系统资源: {}",
                    stale_id
                );
                self.open_pdf_tabs.insert(stale_id, None);
                cx.notify();
            }
        }
    }

    fn render_main_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let ui_state = cx.global::<UiState>();
        let view_mode = ui_state.view_mode;
        let has_selected_id = if view_mode == AppViewMode::Library {
            !ui_state.selected_literature_ids.is_empty()
        } else {
            !ui_state.selected_feed_item_ids.is_empty()
        };
        let left_width = self.left_width;
        let right_width = self.right_width;

        div()
            .flex()
            .flex_row()
            .flex_grow()
            .h_0()
            .relative()
            // 1. 左侧边栏
            .child(div().h_full().w(left_width).flex_shrink_0().child(
                if view_mode == AppViewMode::Library {
                    self.literature_panel.clone().into_any_element()
                } else {
                    self.subscription_panel.clone().into_any_element()
                },
            ))
            // 2. 主区域 — v_flex: bar + content + dropdowns
            .child(
                v_flex()
                    .flex_grow()
                    .h_full()
                    .relative()
                    .child(
                        self.toolbar_view
                            .update(cx, |tb, cx| tb.render_bar(window, cx)),
                    )
                    .child(
                        h_flex()
                            .flex_grow()
                            .h_0()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .flex_grow()
                                    .flex_shrink()
                                    .min_w(rems(0.0))
                                    .overflow_hidden()
                                    .child(if view_mode == AppViewMode::Library {
                                        self.literature_list.clone().into_any_element()
                                    } else {
                                        self.subscription_list.clone().into_any_element()
                                    }),
                            )
                            .when(has_selected_id, |this: gpui::Div| {
                                this.child(div().h_full().w(right_width).flex_shrink_0().child(
                                    if view_mode == crate::services::AppViewMode::Library {
                                        self.literature_detail.clone().into_any_element()
                                    } else {
                                        self.subscription_detail.clone().into_any_element()
                                    },
                                ))
                            })
                            .when(has_selected_id, |this: gpui::Div| {
                                this.child(layout::render_right_resizer(right_width, cx))
                            }),
                    )
                    .children(
                        self.toolbar_view
                            .update(cx, |tb, cx| tb.render_dropdowns(cx)),
                    ),
            )
            // 3. 调节条
            .child(layout::render_left_resizer(left_width, cx))
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.current_window_width = window.bounds().size.width;
        self.current_window_height = window.bounds().size.height;

        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .on_action(cx.listener(|this, _: &HandleSyncConflicts, _window, cx| {
                this.handle_sync_conflicts(cx);
            }))
            .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                this.loading_modal = None;
                this.context_menu = None;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ShowAbout, _window, cx| {
                this.open_settings_modal(cx, Some(SettingsTab::About));
            }))
            .on_action(cx.listener(|this, _: &EmptyTrash, _window, cx| {
                this.handle_empty_trash(cx);
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.dragging_left {
                    this.left_width = event
                        .position
                        .x
                        .max(window.rem_size() * 9.375)
                        .min(window.rem_size() * 28.125);
                    cx.notify();
                } else if this.dragging_right {
                    let window_width = this.current_window_width;
                    this.right_width = (window_width - event.position.x)
                        .max(window.rem_size() * 9.375)
                        .min(window.rem_size() * 28.125);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if (this.dragging_left || this.dragging_right)
                        && let Ok(mut state) = this.app.local_state.write()
                    {
                        state.left_sidebar_width = Some(f64::from(f32::from(this.left_width)));
                        state.right_sidebar_width = Some(f64::from(f32::from(this.right_width)));
                    }
                    this.dragging_left = false;
                    this.dragging_right = false;
                    cx.notify();
                }),
            )
            // 1. 顶部标签栏
            .child(self.render_tab_bar(window, cx))
            // 2. 内容区
            .child(match self.active_tab.clone() {
                TabId::Main => self.render_main_content(window, cx).into_any_element(),
                TabId::Pdf(doc_id) => {
                    if let Some(Some(view)) = self.open_pdf_tabs.get(&doc_id) {
                        view.clone().into_any_element()
                    } else {
                        self.active_tab = TabId::Main;
                        self.render_main_content(window, cx).into_any_element()
                    }
                }
            })
            // 3. 菜单遮罩
            .children((self.context_menu.is_some()).then(|| {
                div()
                    .absolute()
                    .size_full()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.context_menu = None;
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, _, _, cx| {
                            this.context_menu = None;
                            cx.notify();
                        }),
                    )
            }))
            // 4. 模态框浮层
            .child(self.toast_overlay.clone())
            .children(
                self.loading_modal
                    .as_ref()
                    .map(|message: &String| modals::render_loading_modal(message.clone(), cx)),
            )
            .children(modals::render_tag_selector(self, window, cx))
            .children(modals::render_folder_selector(self, window, cx))
            .children(self.render_global_context_menu(cx))
            .children((self.active_popup_count > 0).then(|| div().absolute().size_full().occlude()))
    }
}
