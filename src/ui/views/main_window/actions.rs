use log::{debug, error, info};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::RUNTIME;

use gpui::prelude::*;
use gpui::{
    AppContext, AsyncApp, Bounds, Pixels, Point, Size, TitlebarOptions, Window, WindowBounds,
    WindowKind, WindowOptions, px, size,
};
use gpui_component::Root;

use crate::ui::{
    components::{
        CitationPopup, DuplicateList, FetchMode, FieldSelection, LiteratureCompare,
        LiteratureEditor, LiteratureFetcher, MetadataSelector, SettingsTab, SettingsWindow,
        SubscriptionEditor, TagSelector,
    },
    views::main_window::types::FetchSource,
};
use database::constructors::create_literature;
use i18n::{I18nKey, Language, t, tf};
use models::{Feed, Literature, LiteratureType};
use pdf::{PdfInitialState, PdfReaderDelegate, PdfReaderView, PdfService};

struct AppPdfDelegate {
    app: Arc<crate::services::MainApp>,
}

impl PdfReaderDelegate for AppPdfDelegate {
    fn get_initial_state(&self, id: String) -> PdfInitialState {
        let translation_original_expanded = self
            .app
            .local_state
            .read()
            .map(|s| s.translation_original_expanded)
            .unwrap_or(true);
        self.app
            .local_state_manager
            .get_pdf_state(&id)
            .ok()
            .flatten()
            .map(|s| PdfInitialState {
                page_index: s.page_index,
                zoom_level: s.zoom_level,
                offset_y: s.offset_y,
                fit_to_width: s.fit_to_width,
                is_left_sidebar_open: s.is_left_sidebar_open,
                is_right_sidebar_open: s.is_right_sidebar_open,
                left_sidebar_width: s.left_sidebar_width,
                right_sidebar_width: s.right_sidebar_width,
                translation_font_size: self.app.config.lock().unwrap().translation.font_size,
                translation_original_expanded,
            })
            .unwrap_or_else(|| PdfInitialState {
                translation_original_expanded,
                translation_font_size: self.app.config.lock().unwrap().translation.font_size,
                ..Default::default()
            })
    }

    fn save_state(
        &self,
        id: String,
        page: u16,
        zoom: f32,
        offset_y: f32,
        fit_to_width: bool,
        is_left_sidebar_open: bool,
        is_right_sidebar_open: bool,
        left_sidebar_width: f32,
        right_sidebar_width: f32,
    ) {
        let lit_id = id.split("::").next().unwrap_or(&id).to_string();
        let path = self
            .app
            .db
            .get_literature(&lit_id)
            .ok()
            .flatten()
            .and_then(|l| {
                l.attachments
                    .iter()
                    .find(|a| a.is_main)
                    .map(|a| a.file_path.clone())
            })
            .unwrap_or_default();

        let _ = self.app.local_state_manager.save_pdf_state(
            &id,
            &path,
            page,
            zoom,
            offset_y,
            fit_to_width,
            is_left_sidebar_open,
            is_right_sidebar_open,
            left_sidebar_width,
            right_sidebar_width,
        );
    }

    fn translate(
        &self,
        text: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>> {
        let app = self.app.clone();
        Box::pin(async move {
            let translation_service = {
                let lock = app.translation_service.lock().unwrap();
                lock.clone()
            };
            let target_lang = app
                .config
                .lock()
                .unwrap()
                .translation
                .target_language
                .clone();

            let handle = crate::RUNTIME
                .spawn(async move { translation_service.translate(&text, &target_lang).await });

            match handle.await {
                Ok(res) => res,
                Err(e) => Err(anyhow::anyhow!("Tokio task failed: {}", e)),
            }
        })
    }

    fn get_translation_engines(&self) -> Vec<String> {
        translate::ENGINES
            .iter()
            .map(|e| e.id.to_string())
            .collect()
    }

    fn set_translation_engine(&self, name: String) {
        let mut config = self.app.config.lock().unwrap().clone();
        if config.translation.engine != name {
            config.translation.engine = name;
            let _ = self.app.update_config(config);
        }
    }

    fn current_translation_engine_id(&self) -> String {
        self.app.config.lock().unwrap().translation.engine.clone()
    }

    fn current_language(&self) -> Language {
        self.app.current_language()
    }

    fn set_translation_font_size(&self, size: f32) {
        let mut config = self.app.config.lock().unwrap().clone();
        if (config.translation.font_size - size).abs() > 0.01 {
            config.translation.font_size = size;
            let _ = self.app.update_config(config);
        }
    }

    fn translation_font_size(&self) -> f32 {
        let config = self.app.config.lock().unwrap();
        config.translation.font_size
    }

    fn load_annotations(&self, id: &str) -> Vec<models::Annotation> {
        self.app.db.load_annotations(id).unwrap_or_default()
    }

    fn save_annotation(&self, annotation: &models::Annotation) {
        let _ = self.app.db.save_annotation(annotation);
    }

    fn delete_annotation(&self, id: &str) {
        let _ = self.app.db.delete_annotation(id);
    }

    fn on_link_click(&self, url: String) {
        crate::ui::views::main_window::utils::open_url(&url);
    }

    fn get_notes(&self, id: &str) -> Option<String> {
        let lit_id = id.split("::").next().unwrap_or(id);
        self.app
            .db
            .get_literature(lit_id)
            .ok()
            .flatten()
            .and_then(|l| l.notes)
    }

    fn save_notes(&self, id: &str, notes: &str) {
        let lit_id = id.split("::").next().unwrap_or(id);
        if let Err(e) = self.app.db.update_literature_notes(lit_id, notes) {
            log::error!("保存笔记失败: {e}");
        }
        self.app.notify_data_changed();
    }

    fn set_translation_original_expanded(&self, expanded: bool) {
        if let Ok(mut state) = self.app.local_state.write() {
            state.translation_original_expanded = expanded;
        }
        let state = self.app.local_state.read().unwrap().clone();
        let _ = self.app.local_state_manager.save_all(&state);
    }
}

impl super::MainWindow {
    pub fn open_pdf_viewer(&mut self, lit: Literature, cx: &mut Context<Self>) {
        self.open_pdf_viewer_with_path(lit, None, cx);
    }

    pub fn open_pdf_viewer_with_path(
        &mut self,
        lit: Literature,
        preferred_path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = preferred_path.or_else(|| {
            lit.attachments
                .iter()
                .find(|a| a.is_main)
                .map(|a| PathBuf::from(&a.file_path))
                .or_else(|| {
                    lit.attachments
                        .iter()
                        .find(|a| a.file_path.to_lowercase().ends_with(".pdf"))
                        .map(|a| PathBuf::from(&a.file_path))
                })
        }) else {
            error!("MainWindow: 该文献没有 PDF 附件");
            return;
        };

        info!("MainWindow: 尝试打开 PDF 阅读器, 路径: {:?}", path);
        if !path.exists() {
            error!("MainWindow: PDF 文件不存在: {:?}", path);
            let lang = self.app.current_language();
            self.open_error_modal(
                t(I18nKey::FileNotFoundTitle, lang),
                tf(I18nKey::FileNotFoundMsg, lang, &[&format!("{:?}", path)]),
                cx,
            );
            return;
        }

        let doc_id = lit
            .attachments
            .iter()
            .find(|a| a.file_path == path.to_string_lossy())
            .map(|a| format!("{}::{}", lit.id, a.id))
            .unwrap_or_else(|| lit.id.clone());

        let app = self.app.clone();
        let doc_id_for_close = doc_id.clone();
        let this_weak = cx.entity().downgrade();

        if self.open_pdf_doc_ids.contains(&doc_id) {
            info!("MainWindow: PDF 阅读器已打开，跳过重复打开: {doc_id}");
            return;
        }
        self.open_pdf_doc_ids.insert(doc_id.clone());

        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: None,
                appears_transparent: true,
                traffic_light_position: Some(Point::new(px(9.0), px(9.0))),
            }),
            window_bounds: Some(WindowBounds::Maximized(Bounds::default())),
            window_min_size: Some(size(px(800.0), px(600.0))),
            ..Default::default()
        };

        cx.open_window(options, move |window, cx| {
            let (pdf_service, response_rx) =
                PdfService::new(path.clone()).expect("Failed to create PdfService");
            let delegate = Arc::new(AppPdfDelegate { app: app.clone() });
            let view = cx.new(|cx| {
                let mut view = PdfReaderView::new(pdf_service, Some(delegate), doc_id, cx);
                view.init_workers(response_rx, cx);
                view
            });
            let root = cx.new(|cx| Root::new(view, window, cx));
            cx.observe_release(&root, move |_, cx| {
                if let Some(this) = this_weak.upgrade() {
                    let _ = this.update(cx, |this, cx| {
                        this.open_pdf_doc_ids.remove(&doc_id_for_close);
                        cx.notify();
                    });
                }
            })
            .detach();
            root
        })
        .expect("Failed to open PDF viewer window");
    }

    pub fn open_error_modal(
        &mut self,
        title: impl Into<String>,
        content: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.error_modal = Some((title.into(), content.into()));
        cx.notify();

        let this_weak = cx.entity().downgrade();
        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(5))
                    .await;

                let _ = this_weak.update(&mut cx, |this, cx| {
                    if this.error_modal.is_some() {
                        this.error_modal = None;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn open_metadata_selector(
        &mut self,
        candidates: Vec<Literature>,
        cx: &mut Context<Self>,
        on_select: impl Fn(&mut Self, Literature, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) {
        let app = self.app.clone();
        let size = size(px(600.0), px(500.0));
        let this_weak = cx.entity().downgrade();

        self.open_modal_window(size, cx, move |_window, _cx| {
            MetadataSelector::new(app, candidates, move |selected, window, cx| {
                if let Some(this) = this_weak.upgrade() {
                    this.update(cx, |this, cx| {
                        if let Some(lit) = selected {
                            on_select(this, lit, window, cx);
                        }
                        cx.notify();
                    });
                }
                window.remove_window();
            })
        });
    }

    pub fn start_fetch_and_compare(
        &mut self,
        original: Literature,
        source: FetchSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let lang = app.current_language();

        self.loading_modal = Some(t(I18nKey::LoadingMetadata, lang).to_string());
        cx.notify();

        let window_handle = window.window_handle();
        let this_weak = cx.entity().downgrade();

        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx_inner = cx.clone();
            async move {
                let result = {
                    let _guard = RUNTIME.enter();
                    match source {
                        FetchSource::ArXiv(id) => app
                            .fetcher_service
                            .parse_arxiv(&id)
                            .await
                            .map(|lit| vec![lit]),
                        FetchSource::Doi(doi) => app
                            .fetcher_service
                            .parse_doi(&doi)
                            .await
                            .map(|lit| vec![lit]),
                        FetchSource::Dblp(query) => app.fetcher_service.search_dblp(&query).await,
                        FetchSource::OpenAlexDoi(doi) => app
                            .fetcher_service
                            .parse_openalex(&doi)
                            .await
                            .map(|lit| vec![lit]),
                        FetchSource::OpenAlexTitle(title) => {
                            app.fetcher_service.search_openalex(&title, 5).await
                        }
                    }
                };

                let _ = cx_inner.update_window(window_handle, |_, _window, cx| {
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            if this.loading_modal.is_none() {
                                return;
                            }
                            this.loading_modal = None;

                            match result {
                                Ok(mut candidates) => {
                                    if candidates.is_empty() {
                                        this.open_error_modal(
                                            t(I18nKey::FetchFailed, lang),
                                            "No results found",
                                            cx,
                                        );
                                    } else if candidates.len() == 1 {
                                        let new_lit = candidates.pop().unwrap();
                                        this.show_literature_compare(original, new_lit, cx);
                                    } else {
                                        let original_clone = original.clone();
                                        this.open_metadata_selector(
                                            candidates,
                                            cx,
                                            move |this, lit, _, cx| {
                                                this.show_literature_compare(
                                                    original_clone.clone(),
                                                    lit,
                                                    cx,
                                                );
                                            },
                                        );
                                    }
                                }
                                Err(e) => {
                                    this.open_error_modal(
                                        t(I18nKey::FetchFailed, lang),
                                        format!("{}: {}", t(I18nKey::FetchFailed, lang), e),
                                        cx,
                                    );
                                }
                            }
                            cx.notify();
                        });
                    }
                });
            }
        })
        .detach();
    }

    fn show_literature_compare(
        &mut self,
        original: Literature,
        new_lit: Literature,
        cx: &mut Context<Self>,
    ) {
        self.show_literature_compare_with_callback(original, new_lit, cx, |_, _| {});
    }

    pub fn show_literature_compare_with_callback(
        &mut self,
        original: Literature,
        new_lit: Literature,
        cx: &mut Context<Self>,
        on_done: impl Fn(&mut Self, &mut Context<Self>) + Send + Sync + 'static,
    ) {
        info!("Metadata Compare Debug - Local Data: {original:?}");
        info!("Metadata Compare Debug - Fetched Data: {new_lit:?}");

        let selection = FieldSelection::compare(&original, &new_lit);

        if !selection.has_any_diff() {
            info!("获取元数据: 结果与本地完全一致，无需合并。");
            let lang = self.app.current_language();
            self.open_error_modal(
                t(I18nKey::DataConsistentTitle, lang),
                t(I18nKey::DataConsistentMsg, lang),
                cx,
            );
            on_done(self, cx);
            return;
        }

        let app = self.app.clone();
        let size = size(px(1100.0), px(800.0));
        let this_weak = cx.entity().downgrade();
        let on_done = Arc::new(on_done);

        self.open_modal_window(size, cx, move |_window, _cx| {
            let on_done_cb = on_done.clone();
            let this_weak_cb = this_weak.clone();

            LiteratureCompare::new_with_data(
                app,
                original,
                new_lit,
                selection,
                move |_, window, cx| {
                    window.remove_window();
                    if let Some(this) = this_weak_cb.upgrade() {
                        this.update(cx, |this, cx| {
                            on_done_cb(this, cx);
                        });
                    }
                },
            )
        });
    }

    pub fn open_tag_selector(
        &mut self,
        current_tags: Vec<String>,
        on_select: impl Fn(String, &mut Window, &mut Context<TagSelector>) + Send + Sync + 'static,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();

        let selector = TagSelector::build(
            app,
            current_tags,
            window,
            cx,
            on_select,
            move |_window, cx| {
                let _ = this_weak.update(cx, |this, cx| {
                    this.tag_selector = None;
                    cx.notify();
                });
            },
        );
        self.tag_selector = Some((selector, position));
        cx.notify();
    }

    pub fn open_citation_selector(
        &mut self,
        exclude_id: String,
        on_select: impl Fn(String, &mut Window, &mut Context<Self>) + Send + Sync + 'static,
        cx: &mut Context<Self>,
    ) {
        let on_select = Arc::new(on_select);

        let candidates = {
            let data = self.data_store.read(cx);
            data.literatures
                .iter()
                .filter(|lit| lit.id != exclude_id)
                .cloned()
                .collect::<Vec<_>>()
        };

        self.open_metadata_selector(candidates, cx, move |_, lit: Literature, window, cx| {
            let on_select = on_select.clone();
            on_select(lit.id, window, cx);
        });
    }

    pub fn open_edit_subscription_modal(&mut self, feed_id: String, cx: &mut Context<Self>) {
        let feed = {
            let data = self.data_store.read(cx);
            data.feeds.iter().find(|f| f.id == feed_id).cloned()
        };

        if let Some(feed) = feed {
            self.show_subscription_editor(feed.into(), cx);
        }
    }

    pub fn open_add_subscription_modal(&mut self, cx: &mut Context<Self>) {
        self.show_subscription_editor(None, cx);
    }

    fn show_subscription_editor(&mut self, feed: Option<Feed>, cx: &mut Context<Self>) {
        let this_weak = cx.entity().downgrade();
        let app = self.app.clone();
        let is_edit = feed.is_some();
        let feed_id = feed.as_ref().map(|f| f.id.clone());
        let size = size(px(400.0), px(320.0));

        self.open_modal_window(size, cx, move |window, cx| {
            SubscriptionEditor::new(
                app.clone(),
                window,
                cx,
                feed,
                move |name, url, interval, window, cx| {
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            let res = if let Some(ref fid) = feed_id {
                                this.app
                                    .clone()
                                    .update_feed(fid.clone(), name, url, interval)
                            } else {
                                this.app.clone().add_feed(name, url, interval)
                            };

                            if let Err(e) = res {
                                error!("{}订阅失败: {}", if is_edit { "更新" } else { "添加" }, e);
                            }
                            cx.notify();
                        });
                    }
                    window.remove_window();
                },
            )
        });
    }

    pub fn open_settings_modal(&mut self, cx: &mut Context<Self>, target_tab: Option<SettingsTab>) {
        info!("UI: 用户打开设置对话框, 目标标签: {target_tab:?}");
        let app = self.app.clone();
        let size = size(px(850.0), px(600.0));

        self.open_modal_window(size, cx, move |window, cx| {
            SettingsWindow::new(app, window, cx, target_tab)
        });
    }
    pub fn open_manual_add_modal(&mut self, cx: &mut Context<Self>) {
        info!("UI: 用户触发手动添加文献");
        let mut lit = create_literature(Uuid::new_v4().to_string(), "", LiteratureType::Article);

        let ui_folder = cx
            .global::<crate::services::ui_state::UiState>()
            .selected_folder_id
            .clone();
        if let Some(folder_id) = &ui_folder
            && folder_id != "all"
            && folder_id != "uncategorized"
            && folder_id != "trash"
        {
            lit.folder_ids.push(folder_id.clone());
        }

        self.show_literature_editor(lit, true, cx);
    }

    pub fn open_fetch_modal(&mut self, mode: FetchMode, cx: &mut Context<Self>) {
        info!("UI: 用户打开文献抓取对话框, 模式: {mode:?}");
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let size = size(px(500.0), px(140.0));

        self.open_modal_window(size, cx, move |window, cx| {
            LiteratureFetcher::new(app, mode, window, cx, move |result, window, cx| {
                debug!(
                    "FETCH_CB: 抓取窗口即将关闭 (result={})",
                    result.as_ref().map_or(0, |v| v.len())
                );
                window.remove_window();
                debug!("FETCH_CB: 抓取窗口已关闭，开始处理导入");

                if let Some(this) = this_weak.upgrade() {
                    this.update(cx, |this, cx| {
                        if let Some(lits) = result {
                            let should_select = lits.len() > 1
                                && (mode == FetchMode::Dblp || mode == FetchMode::OpenAlex);

                            if should_select {
                                debug!("FETCH_CB: 打开选择器 ({}条)", lits.len());
                                this.open_metadata_selector(
                                    lits,
                                    cx,
                                    |this, lit: Literature, _window, _cx| {
                                        this.pending_imports.push(lit);
                                    },
                                );
                            } else {
                                debug!("FETCH_CB: 直接推入编辑器队列 ({}条)", lits.len());
                                this.pending_imports.extend(lits);
                            }
                        } else {
                            debug!("FETCH_CB: result=None，不处理");
                        }
                        cx.notify();
                    });
                } else {
                    debug!("FETCH_CB: MainWindow 已释放，无法处理导入");
                }
            })
        });
    }

    fn process_next_pending_import(&mut self, cx: &mut Context<Self>) {
        if self.pending_imports.is_empty() {
            return;
        }

        let lit = self.pending_imports.remove(0);
        info!(
            "UI: 处理批量导入队列，剩余 {} 条，正在打开编辑器: {}",
            self.pending_imports.len(),
            lit.title
        );
        self.show_literature_editor(lit, true, cx);
    }

    pub(super) fn show_literature_editor(
        &mut self,
        lit: Literature,
        is_new: bool,
        cx: &mut Context<Self>,
    ) {
        debug!("EDITOR: show_literature_editor 进入 (is_new={})", is_new);
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let size = size(px(600.0), px(700.0));

        self.open_modal_window(size, cx, move |window, cx| {
            debug!("EDITOR: open_modal_window 回调执行，创建 LiteratureEditor");
            LiteratureEditor::new(app, lit, window, cx, move |result, window, cx| {
                debug!("EDITOR: 编辑器回调触发 (result={})", result.is_some());
                if let Some(this) = this_weak.upgrade() {
                    this.update(cx, |this, cx| {
                        if let Some(lit) = result {
                            if is_new {
                                debug!("EDITOR: 调用 confirm_add_literature");
                                this.confirm_add_literature(lit, cx);
                            } else {
                                debug!("EDITOR: 调用 confirm_edit_literature");
                                this.confirm_edit_literature(lit, cx);
                            }
                        } else {
                            debug!("EDITOR: 用户取消");
                        }
                        cx.notify();

                        debug!("EDITOR: 回调处理完毕");
                    });
                }
                debug!("EDITOR: 关闭编辑器窗口");
                window.remove_window();
            })
        });
    }

    pub fn open_edit_modal(&mut self, target_id: Option<String>, cx: &mut Context<Self>) {
        let lit = if let Some(id) = target_id {
            {
                let data = self.data_store.read(cx);
                data.literatures.iter().find(|l| l.id == id).cloned()
            }
        } else {
            let first_id = cx
                .global::<crate::services::ui_state::UiState>()
                .selected_literature_ids
                .iter()
                .next()
                .cloned();
            if let Some(id) = first_id {
                {
                    let data = self.data_store.read(cx);
                    data.literatures.iter().find(|l| l.id == id).cloned()
                }
            } else {
                None
            }
        };

        if let Some(lit) = lit {
            self.show_literature_editor(lit, false, cx);
        }
    }

    fn confirm_add_literature(&mut self, mut lit: Literature, cx: &mut Context<Self>) {
        info!("业务: 用户确认添加新文献: {}", lit.title);
        let ui_folder = cx
            .global::<crate::services::ui_state::UiState>()
            .selected_folder_id
            .clone();
        if let Some(folder_id) = &ui_folder
            && folder_id != "all"
            && folder_id != "uncategorized"
            && folder_id != "trash"
        {
            lit.folder_ids.push(folder_id.clone());
        }

        match self.app.add_literature(lit.clone()) {
            Ok(()) => {
                info!("成功添加文献: {}", lit.title);
            }
            Err(e) => {
                error!("添加文献失败: {e}");
            }
        }
        cx.notify();
    }

    fn confirm_edit_literature(&mut self, lit: Literature, cx: &mut Context<Self>) {
        info!("业务: 用户确认更新文献: {} (ID: {})", lit.title, lit.id);
        match self.app.update_literature(lit.clone()) {
            Ok(()) => {
                info!("成功更新文献: {}", lit.title);
            }
            Err(e) => {
                error!("更新文献失败: {e}");
            }
        }
        cx.notify();
    }

    pub fn open_citation_popup(&mut self, cx: &mut Context<Self>) {
        let app = self.app.clone();
        let size = size(px(700.0), px(500.0));
        let selected_ids = cx
            .global::<crate::services::ui_state::UiState>()
            .selected_literature_ids
            .clone();

        self.open_modal_window(size, cx, move |window, cx| {
            CitationPopup::new(app, selected_ids.clone(), window, cx)
        });
    }

    pub fn run_duplicate_detection(&mut self, cx: &mut Context<Self>) {
        let groups = self.app.find_duplicates();
        let lang = self.app.current_language();

        if groups.is_empty() {
            self.open_error_modal(
                t(I18nKey::DuplicateGroups, lang),
                t(I18nKey::NoDuplicatesFound, lang),
                cx,
            );
            return;
        }

        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let size = size(px(700.0), px(600.0));

        self.open_modal_window(size, cx, move |_window, _cx| {
            DuplicateList::new(
                app,
                groups.clone(),
                move |selected_idx, window, cx| {
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            if let Some(idx) = selected_idx {
                                let group = groups[idx].clone();
                                this.start_merge_flow(group, cx);
                            }
                            cx.notify();
                        });
                    }
                    window.remove_window();
                },
                false,
            )
        });
    }

    pub fn handle_sync_conflicts(&mut self, cx: &mut Context<Self>) {
        let groups = if let Ok(state) = self.app.sync_state.lock() {
            state.sync_conflict_groups.clone()
        } else {
            None
        };

        if let Some(groups) = groups {
            let app = self.app.clone();
            let this_weak = cx.entity().downgrade();
            let groups_clone = groups.clone();
            let size = size(px(700.0), px(600.0));

            self.open_modal_window(size, cx, move |_window, _cx| {
                DuplicateList::new(
                    app,
                    groups_clone,
                    move |selected_idx, window, cx| {
                        if let Some(this) = this_weak.upgrade() {
                            this.update(cx, |this, cx| {
                                if let Some(idx) = selected_idx {
                                    let group = groups[idx].clone();
                                    this.start_sync_conflict_resolve_flow(group, cx);
                                } else {
                                    if let Ok(mut state) = this.app.sync_state.lock() {
                                        state.sync_conflict_groups = None;
                                        if matches!(
                                            state.sync_status,
                                            crate::services::SyncStatus::Conflict(_)
                                        ) {
                                            state.sync_status = crate::services::SyncStatus::Idle;
                                        }
                                    }
                                }
                                cx.notify();
                            });
                        }
                        window.remove_window();
                    },
                    true,
                )
            });
        }
    }

    fn start_sync_conflict_resolve_flow(&mut self, group: Vec<Literature>, cx: &mut Context<Self>) {
        if group.len() < 2 {
            return;
        }

        let local_lit = group[0].clone();
        let remote_lit = group[1].clone();
        self.resolve_next_sync_conflict(local_lit, remote_lit, cx);
    }

    fn open_modal_window<V: Render>(
        &mut self,
        size: Size<Pixels>,
        cx: &mut Context<Self>,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V + Send + 'static,
    ) {
        if self.active_popup_count > 0 {
            debug!("MODAL: 已有活跃弹窗，跳过 (size={:?})", size);
            return;
        }
        let bounds = Bounds::centered(None, size, cx);
        debug!("MODAL: 开始创建窗口 (size={:?}, bounds={:?})", size, bounds);

        self.active_popup_count += 1;
        cx.notify();

        let this_weak = cx.entity().downgrade();
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: None,
                    appears_transparent: true,
                    traffic_light_position: Some(Point::new(px(9.0), px(9.0))),
                }),
                is_resizable: false,
                is_minimizable: false,
                kind: WindowKind::Floating,
                ..Default::default()
            },
            move |window, cx| {
                debug!("MODAL: open_window 内部回调执行 (build_view)");
                let view = cx.new(|cx| build_view(window, cx));
                let root = cx.new(|cx| Root::new(view, window, cx));

                cx.observe_release(&root, move |_, cx| {
                    debug!("MODAL: 窗口根组件已释放");
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            this.active_popup_count = this.active_popup_count.saturating_sub(1);
                            if this.active_popup_count == 0 {
                                this.process_next_pending_import(cx);
                            }
                            cx.notify();
                        });
                    }
                })
                .detach();
                root
            },
        );

        if let Err(e) = result {
            error!("MODAL: 窗口创建失败分支 (重复): {e}");
            self.active_popup_count = self.active_popup_count.saturating_sub(1);
            cx.notify();
        }
    }

    fn resolve_next_sync_conflict(
        &mut self,
        local_lit: Literature,
        remote_lit: Literature,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();

        let selection = FieldSelection::compare(&local_lit, &remote_lit);
        let size = size(px(1100.0), px(800.0));

        self.open_modal_window(size, cx, move |_window, _cx| {
            let this_weak_cb = this_weak.clone();
            let remote_ver = remote_lit.version;

            LiteratureCompare::new_with_data(
                app.clone(),
                local_lit.clone(),
                remote_lit.clone(),
                selection,
                move |result, window: &mut Window, cx| {
                    if let Some(this) = this_weak_cb.upgrade() {
                        this.update(cx, |this, cx| {
                            if let Some(mut merged) = result {
                                info!(
                                    "冲突解决: 确认合并。手动提升版本号至 {} 以覆盖远程版本 {}",
                                    remote_ver + 1,
                                    remote_ver
                                );
                                merged.version = remote_ver + 1;
                                merged.is_dirty = true;
                                if let Err(e) = this.app.update_literature(merged) {
                                    error!("冲突解决: 更新本地文献失败: {e}");
                                }
                            } else {
                                info!(
                                    "冲突解决: 用户取消/保留本地。强制提升本地版本号以覆盖远程。"
                                );
                                let mut local_fixed = local_lit.clone();
                                local_fixed.version = remote_ver + 1;
                                local_fixed.is_dirty = true;
                                if let Err(e) = this.app.update_literature(local_fixed) {
                                    error!("冲突解决: 强制更新本地版本失败: {e}");
                                }
                            }

                            let mut should_reopen = false;
                            if let Ok(mut state) = this.app.sync_state.lock()
                                && let Some(groups) = &mut state.sync_conflict_groups
                            {
                                groups.retain(|g| g[0].id != local_lit.id);
                                if groups.is_empty() {
                                    state.sync_conflict_groups = None;
                                    if matches!(
                                        state.sync_status,
                                        crate::services::SyncStatus::Conflict(_)
                                    ) {
                                        state.sync_status = crate::services::SyncStatus::Idle;
                                    }
                                } else {
                                    should_reopen = true;
                                }
                            }
                            if should_reopen {
                                this.handle_sync_conflicts(cx);
                            }
                            cx.notify();
                        });
                    }
                    window.remove_window();
                },
            )
        });
    }

    fn start_merge_flow(&mut self, mut group: Vec<Literature>, cx: &mut Context<Self>) {
        if group.len() < 2 {
            return;
        }

        let original = group.remove(0);
        self.merge_next_in_group(original, group, cx);
    }

    fn merge_next_in_group(
        &mut self,
        original: Literature,
        mut remaining: Vec<Literature>,
        cx: &mut Context<Self>,
    ) {
        if remaining.is_empty() {
            return;
        }

        let next_lit = remaining.remove(0);
        let next_lit_id = next_lit.id.clone();

        let selection = FieldSelection::compare(&original, &next_lit);

        if !selection.has_any_diff() {
            info!("查重合并: 发现完全一致的副本 {next_lit_id}, 正在自动合并并继续...");
            if let Err(e) = self
                .app
                .merge_literature_relations(&next_lit_id, &original.id)
            {
                error!("合并流程: 自动合并关联关系失败: {e}");
            }

            if let Err(e) = self.app.delete_literature_by_id(&next_lit_id) {
                error!("合并流程: 自动合并副本失败: {e}");
            }

            let original_clone = original.clone();
            let remaining_clone = remaining.clone();

            let lang = self.app.current_language();
            self.open_error_modal(
                t(I18nKey::LiteratureMergedTitle, lang),
                tf(I18nKey::LiteratureMergedMsg, lang, &[&next_lit.title]),
                cx,
            );

            self.continue_merge_flow(original_clone, remaining_clone, cx);
            return;
        }

        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let size = size(px(1100.0), px(800.0));

        self.open_modal_window(size, cx, move |_window, _cx| {
            let this_weak_cb = this_weak.clone();
            let app_cb = app.clone();
            let remaining_cb = remaining.clone();
            let next_lit_id_cb = next_lit_id.clone();
            let original_cb = original.clone();

            LiteratureCompare::new_with_data(
                app.clone(),
                original.clone(),
                next_lit.clone(),
                selection,
                move |result, window: &mut Window, cx| {
                    if let Some(this) = this_weak_cb.upgrade() {
                        if let Some(merged) = result {
                            info!(
                                "合并流程: 确认保存。正在合并关联关系并将副本 {next_lit_id_cb} 移至回收站..."
                            );

                            if let Err(e) = app_cb.merge_literature_relations(&next_lit_id_cb, &original_cb.id) {
                                error!("合并流程: 合并关联关系失败: {e}");
                            }

                            if let Err(e) = app_cb.delete_literature_by_id(&next_lit_id_cb) {
                                error!("合并流程: 移动副本到回收站失败: {e}");
                            }

                            this.update(cx, |this, cx| {
                                this.continue_merge_flow(merged, remaining_cb.clone(), cx);
                            });
                        } else {
                            info!("合并流程: 跳过当前副本。");
                            this.update(cx, |this, cx| {
                                this.continue_merge_flow(
                                    original_cb.clone(),
                                    remaining_cb.clone(),
                                    cx,
                                );
                            });
                        }
                    }
                    window.remove_window();
                },
            )
        });
    }

    fn continue_merge_flow(
        &mut self,
        original: Literature,
        remaining: Vec<Literature>,
        cx: &mut Context<Self>,
    ) {
        if remaining.is_empty() {
            return;
        }

        let this_weak = cx.entity().downgrade();
        cx.spawn(move |_, cx: &mut gpui::AsyncApp| {
            let cx = cx.clone();
            async move {
                gpui::Timer::after(std::time::Duration::from_millis(150)).await;
                let _ = cx.update(|cx| {
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            this.merge_next_in_group(original, remaining, cx);
                        });
                    }
                });
            }
        })
        .detach();
    }

    pub fn handle_empty_trash(&mut self, cx: &mut Context<Self>) {
        info!("UI: handle_empty_trash triggered");
        let app = self.app.clone();
        cx.spawn(move |_, _cx: &mut gpui::AsyncApp| async move {
            info!("Async Task: Starting empty_trash logic");
            if let Err(e) = app.empty_trash() {
                error!("清空回收站失败: {e}");
            } else {
                info!("Async Task: empty_trash completed successfully");
            }
        })
        .detach();
    }
}
