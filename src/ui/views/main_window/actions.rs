use log::{debug, error, info};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use gpui::prelude::*;
use gpui::{
    AppContext, AsyncApp, Bounds, Pixels, Point, Size, TitlebarOptions, Window, WindowBounds,
    WindowKind, WindowOptions, px, size,
};
use gpui_component::Root;
use gpui_component::WindowExt;
use gpui_component::dialog::DialogButtonProps;

use crate::notification_bus::show_notification;

use crate::ui::{
    components::{
        CitationPopup, FieldSelection, LiteratureCompare, LiteratureEditor, MetadataSelector,
        SubscriptionEditor, TagSelector,
        setting::{SettingsTab, SettingsWindow},
    },
    dialogs::{DuplicateListDialogContent, FetchDialogContent, FetchMode},
    views::main_window::types::FetchSource,
};
use ai::ChatRole;
use database::constructors::create_literature;
use gpui_component::notification::NotificationType;
use i18n::{I18nKey, Language, t, tf};
use models::{Feed, Literature, LiteratureType};
use pdf::{AiBackendItem, PdfInitialState, PdfReaderDelegate};

pub(crate) struct AppPdfDelegate {
    pub(crate) app: Arc<crate::services::MainApp>,
    pub(crate) literature_id: String,
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
                auto_translate: s.auto_translate,
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
        auto_translate: bool,
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

        if let Err(e) = self.app.local_state_manager.save_pdf_state(
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
            auto_translate,
        ) {
            log::error!("Failed to save PDF state: {:?}", e);
        }
    }

    fn translate(
        &self,
        text: String,
        force: bool,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send>> {
        let app = self.app.clone();
        Box::pin(async move {
            let (translation_service, current_engine) = {
                let lock = app.translation_service.lock().unwrap();
                let engine = app.config.lock().unwrap().translation.engine.clone();
                debug!("开始翻译，当前配置引擎: {}", engine);
                (lock.clone(), engine)
            };
            let target_lang = app
                .config
                .lock()
                .unwrap()
                .translation
                .target_language
                .clone();
            debug!(
                "使用引擎={}, 目标语言={}, 文本长度={}, 强制={}",
                current_engine,
                target_lang,
                text.len(),
                force
            );

            let handle = crate::RUNTIME.spawn(async move {
                translation_service
                    .translate(&text, &target_lang, force)
                    .await
            });

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

    fn set_translation_engine(&self, name: String, cx: &mut gpui::App) {
        let current = self.app.config.lock().unwrap().translation.engine.clone();
        debug!("请求切换引擎: {} -> {}", current, name);
        let mut config = self.app.config.lock().unwrap().clone();
        if config.translation.engine != name {
            debug!("条件满足，开始切换");
            config.translation.engine = name.clone();
            match self.app.update_config(config) {
                Ok(_) => {
                    let after = self.app.config.lock().unwrap().translation.engine.clone();
                    debug!("update_config 成功，切换后配置引擎: {}", after);
                }
                Err(e) => {
                    error!("update_config 失败: {}", e);
                }
            }
            cx.update_global::<crate::config_store::ConfigStore, _>(|store, _cx| {
                debug!(
                    "更新 ConfigStore，旧值: {}, 新值: {}",
                    store.inner.translation.engine, name
                );
                store.inner.translation.engine = name;
            });
        } else {
            debug!("引擎未变化，跳过切换 (当前={})", current);
        }
    }

    fn current_translation_engine_id(&self) -> String {
        let id = self.app.config.lock().unwrap().translation.engine.clone();
        debug!("current_translation_engine_id 返回: {}", id);
        id
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

    fn list_notes(&self, literature_id: &str) -> Vec<models::LiteratureNote> {
        self.app.db.list_notes(literature_id).unwrap_or_default()
    }

    fn create_note(&self, literature_id: &str, title: &str) -> Option<String> {
        let id = self.app.db.create_note(literature_id, title).ok()?;
        self.app.notify_data_changed();
        Some(id)
    }

    fn update_note(&self, note_id: &str, title: Option<&str>, content: Option<&str>) -> bool {
        let ok = self
            .app
            .db
            .update_note(note_id, title, content)
            .unwrap_or(false);
        if ok {
            self.app.notify_data_changed();
        }
        ok
    }

    fn delete_note(&self, note_id: &str) -> bool {
        let ok = self.app.db.delete_note(note_id).unwrap_or(false);
        if ok {
            self.app.notify_data_changed();
        }
        ok
    }

    // ── AI 对话 ─────────────────────────────────────────

    fn list_chat_sessions(&self, literature_id: &str) -> Vec<models::chat::ChatSession> {
        self.app
            .local_state_manager
            .list_chat_sessions(literature_id)
            .unwrap_or_default()
    }

    fn create_chat_session(
        &self,
        literature_id: &str,
        title: &str,
        system_prompt: &str,
    ) -> Option<String> {
        self.app
            .local_state_manager
            .create_chat_session(literature_id, title, system_prompt)
            .ok()
    }

    fn delete_chat_session(&self, session_id: &str) -> bool {
        self.app
            .local_state_manager
            .delete_chat_session(session_id)
            .unwrap_or(false)
    }

    fn update_chat_session(
        &self,
        session_id: &str,
        title: Option<&str>,
        system_prompt: Option<&str>,
    ) -> bool {
        self.app
            .local_state_manager
            .update_chat_session(session_id, title, system_prompt)
            .unwrap_or(false)
    }

    fn list_chat_messages(&self, session_id: &str) -> Vec<models::chat::ChatMessage> {
        self.app
            .local_state_manager
            .get_chat_message_chain(session_id)
            .unwrap_or_default()
    }

    fn current_literature_attachments(&self) -> Vec<models::Attachment> {
        self.app
            .db
            .get_all_literatures()
            .unwrap_or_default()
            .into_iter()
            .find(|l| l.id == self.literature_id)
            .map(|l| l.attachments)
            .unwrap_or_default()
    }

    fn add_chat_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        attachments: &[String],
        reasoning: Option<&str>,
    ) -> Option<String> {
        self.app
            .local_state_manager
            .add_chat_message(session_id, role, content, attachments, reasoning)
            .ok()
    }

    fn add_chat_message_with_parent(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        attachments: &[String],
        reasoning: Option<&str>,
        parent_id: Option<&str>,
    ) -> Option<String> {
        self.app
            .local_state_manager
            .add_chat_message_with_parent(
                session_id,
                role,
                content,
                attachments,
                reasoning,
                parent_id,
            )
            .ok()
    }

    fn get_message_siblings(&self, message_id: &str) -> Vec<String> {
        self.app
            .local_state_manager
            .get_message_siblings(message_id)
            .unwrap_or_default()
    }

    fn switch_active_message(&self, session_id: &str, leaf_message_id: &str) -> Result<(), String> {
        self.app
            .local_state_manager
            .switch_active_message(session_id, leaf_message_id)
            .map_err(|e| e.to_string())
    }

    fn find_deepest_leaf(&self, start_message_id: &str) -> Result<String, String> {
        self.app
            .local_state_manager
            .find_deepest_leaf(start_message_id)
            .map_err(|e| e.to_string())
    }

    fn truncate_chat_messages_after(
        &self,
        session_id: &str,
        target_message_id: &str,
    ) -> Result<(), String> {
        self.app
            .local_state_manager
            .truncate_chat_messages_after(session_id, target_message_id)
            .map_err(|e| e.to_string())
    }

    fn chat_stream(
        &self,
        _session_id: String,
        messages: Vec<models::chat::ChatMessage>,
        system_prompt: String,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = std::result::Result<
                        tokio::sync::mpsc::UnboundedReceiver<models::chat::ChatResponseChunk>,
                        String,
                    >,
                > + Send,
        >,
    > {
        let app = self.app.clone();
        Box::pin(async move {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let tx_err = tx.clone();

            let handle = crate::RUNTIME.spawn(async move {
                let keys = app.local_state.read().unwrap().translation_keys.clone();
                let entries_json = keys.get("ai.entries").cloned().unwrap_or_default();
                let active_name = keys.get("chat.active").cloned().unwrap_or_default();
                let entries: Vec<ai::AiBackendEntry> =
                    serde_json::from_str(&entries_json).unwrap_or_default();
                debug!(
                    "[chat_stream] entries={}, active={:?}",
                    entries.len(),
                    active_name
                );
                let entry = entries
                    .iter()
                    .find(|e| e.name == active_name)
                    .ok_or_else(|| {
                        let msg = format!(
                            "未配置 AI 后端 (active={active_name:?}, entries={})",
                            entries.len()
                        );
                        debug!("[chat_stream] {msg}");
                        msg
                    })?;
                debug!(
                    "[chat_stream] found entry: kind={}, model={}",
                    entry.kind, entry.model
                );
                let kind = ai::BackendKind::from_str(&entry.kind);
                let config = entry.to_config();

                let service = ai::AiService::new(kind, &config);

                let is_claude = kind == ai::BackendKind::Claude;

                let chat_msgs: Vec<ai::ChatMessage> = messages
                    .into_iter()
                    .map(|m| {
                        let is_quote = m.role == "quote";
                        let role = match m.role.as_str() {
                            "user" | "quote" => ChatRole::User,
                            "assistant" => ChatRole::Assistant,
                            "system" => ChatRole::System,
                            _ => ChatRole::User,
                        };
                        let content = if is_quote {
                            format!("[引用自文献]\n> {}", m.content)
                        } else {
                            m.content
                        };
                        let attachments: Vec<ai::AttachmentInfo> = m
                            .attachments
                            .iter()
                            .map(|fp| {
                                let file_name = std::path::Path::new(fp)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(fp)
                                    .to_string();
                                let mime_type = if fp.ends_with(".pdf") {
                                    Some("application/pdf".to_string())
                                } else {
                                    None
                                };
                                let extracted_text = if !is_claude {
                                    pdf::extract_text_from_pdf(fp).ok()
                                } else {
                                    None
                                };
                                ai::AttachmentInfo {
                                    file_path: fp.clone(),
                                    file_name,
                                    mime_type,
                                    extracted_text,
                                }
                            })
                            .collect();
                        ai::ChatMessage {
                            role,
                            content,
                            attachments,
                        }
                    })
                    .collect();

                let system = if system_prompt.is_empty() {
                    None
                } else {
                    Some(system_prompt.as_str())
                };

                let existing_summary = app
                    .local_state_manager
                    .get_chat_session_summary(&_session_id)
                    .unwrap_or_default();

                let strategy = ai::compression::create_strategy(&entry.compression_strategy);
                let result = ai::compression::compress_messages(
                    &chat_msgs,
                    &system_prompt,
                    kind,
                    entry.context_window as usize,
                    entry.max_tokens as usize,
                    strategy.as_ref(),
                    &existing_summary,
                    &service,
                )
                .await
                .map_err(|e| e.to_string())?;

                // Persist any new summary from compression.
                if let Some(ref new_summary) = result.new_summary {
                    let _ = app
                        .local_state_manager
                        .update_chat_session_summary(&_session_id, new_summary);
                }

                let stream = service
                    .chat_stream(&result.messages, system)
                    .await
                    .map_err(|e| e.to_string())?;

                use futures_util::StreamExt;
                let mut stream = stream;
                while let Some(result) = stream.next().await {
                    match result {
                        Ok(token) => {
                            if tx.send(token).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("AI chat stream error: {e}");
                            let _ = tx.send(models::chat::ChatResponseChunk::Content(format!(
                                "\n[AI 流错误: {e}]"
                            )));
                            break;
                        }
                    }
                }

                Ok::<_, String>(())
            });

            drop(crate::RUNTIME.spawn(async move {
                match handle.await {
                    Ok(Err(business_err)) => {
                        error!("[chat_stream] business failed: {business_err}");
                        let _ = tx_err.send(models::chat::ChatResponseChunk::Content(format!(
                            "AI 错误: {business_err}"
                        )));
                    }
                    Err(join_err) => {
                        error!("[chat_stream] task panicked: {join_err:?}");
                        let _ = tx_err.send(models::chat::ChatResponseChunk::Content(format!(
                            "AI 错误 (任务崩溃): {join_err}"
                        )));
                    }
                    Ok(Ok(())) => {}
                }
            }));

            Ok(rx)
        })
    }

    fn list_ai_backends(&self) -> Vec<AiBackendItem> {
        let keys = self
            .app
            .local_state
            .read()
            .unwrap()
            .translation_keys
            .clone();
        let entries_json = keys.get("ai.entries").cloned().unwrap_or_default();
        let entries: Vec<ai::AiBackendEntry> =
            serde_json::from_str(&entries_json).unwrap_or_default();
        entries
            .iter()
            .map(|e| AiBackendItem {
                name: e.name.clone(),
                kind: e.kind.clone(),
                model: e.model.clone(),
            })
            .collect()
    }

    fn get_active_chat_backend(&self) -> Option<String> {
        let keys = self
            .app
            .local_state
            .read()
            .unwrap()
            .translation_keys
            .clone();
        keys.get("chat.active").cloned().filter(|s| !s.is_empty())
    }

    fn set_active_chat_backend(&self, name: &str) {
        let mut local_state = self.app.local_state.write().unwrap();
        let mut keys = local_state.translation_keys.clone();
        keys.insert("chat.active".to_string(), name.to_string());
        local_state.translation_keys = keys;
        let _ = self.app.local_state_manager.save_all(&local_state);
    }

    fn set_translation_original_expanded(&self, expanded: bool) {
        if let Ok(mut state) = self.app.local_state.write() {
            state.translation_original_expanded = expanded;
        }
        let state = self.app.local_state.read().unwrap().clone();
        let _ = self.app.local_state_manager.save_all(&state);
    }

    fn get_page_color_mode(&self) -> String {
        self.app
            .local_state
            .read()
            .map(|s| {
                s.pdf_page_color_mode
                    .clone()
                    .unwrap_or_else(|| "white".to_string())
            })
            .unwrap_or_else(|_| "white".to_string())
    }

    fn set_page_color_mode(&self, mode: String) {
        if let Ok(mut state) = self.app.local_state.write() {
            state.pdf_page_color_mode = Some(mode);
        }
        if let Ok(state) = self.app.local_state.read() {
            let _ = self.app.local_state_manager.save_all(&state);
        }
    }
}

impl super::MainWindow {
    pub fn open_pdf_viewer(&mut self, lit: Arc<Literature>, cx: &mut Context<Self>) {
        self.open_pdf_viewer_with_path(lit, None, cx);
    }

    pub fn open_pdf_viewer_with_path(
        &mut self,
        lit: Arc<Literature>,
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
            show_notification(
                NotificationType::Error,
                format!(
                    "{}: {}",
                    t(I18nKey::FileNotFoundTitle, lang),
                    tf(I18nKey::FileNotFoundMsg, lang, &[&format!("{:?}", path)])
                ),
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

        // 如果已经在标签页中，直接切换并激活
        if self.open_pdf_tabs.contains_key(&doc_id) {
            info!("MainWindow: PDF 已在标签页中，切换与激活: {doc_id}");
            self.activate_pdf_tab(doc_id.clone(), cx);
            return;
        }

        // 记录重新加载用的元数据与已计算确定的确切 PDF 路径
        self.pdf_tab_titles
            .insert(doc_id.clone(), lit.title.clone());
        self.pdf_tab_paths
            .insert(doc_id.clone(), (lit.clone(), Some(path.clone())));

        // 在标签管理中预留空占位（以便可以在顶部顺利渲染标签占位）
        self.open_pdf_tabs.insert(doc_id.clone(), None);
        self.open_pdf_tab_order.push(doc_id.clone());

        // 激活并懒加载当前 PDF
        self.activate_pdf_tab(doc_id, cx);
    }

    fn show_literature_compare(
        &mut self,
        original: Arc<Literature>,
        new_lit: Literature,
        cx: &mut Context<Self>,
    ) {
        self.show_literature_compare_with_callback(original, new_lit, cx, |_, _| {});
    }

    pub fn show_literature_compare_with_callback(
        &mut self,
        original: Arc<Literature>,
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
            show_notification(
                NotificationType::Info,
                format!(
                    "{}: {}",
                    t(I18nKey::DataConsistentTitle, lang),
                    t(I18nKey::DataConsistentMsg, lang)
                ),
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

    pub fn open_metadata_selector(
        &mut self,
        candidates: Vec<Arc<Literature>>,
        cx: &mut Context<Self>,
        on_select: impl Fn(&mut Self, Literature, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let on_select = Arc::new(on_select);
        let size = size(px(500.0), px(400.0));

        self.open_modal_window(size, cx, move |_window, _cx| {
            MetadataSelector::new(app, candidates, move |result, window, cx| {
                if let Some(lit) = result
                    && let Some(this) = this_weak.upgrade()
                {
                    let on_select = on_select.clone();
                    this.update(cx, |this, cx| {
                        on_select(this, lit, window, cx);
                    });
                }
                window.remove_window();
            })
        });
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
            self.show_subscription_editor(Some((*feed).clone()), cx);
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

    pub fn open_fetch_modal(
        &mut self,
        mode: FetchMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("UI: 用户打开文献抓取对话框 (Dialog 版), 模式: {mode:?}");
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let window_handle = window.window_handle();

        // 1. 创建 FetchDialogContent 实体，管理抓取状态
        let entity =
            cx.new(|cx| FetchDialogContent::new(app.clone(), mode, window_handle, window, cx));

        // 2. 订阅 Enter 键触发抓取
        entity.update(cx, |fc, cx| {
            let input = fc.input_entity().clone();
            cx.subscribe(&input, move |fc, _, event, cx| {
                if let gpui_component::input::InputEvent::PressEnter { .. } = event {
                    fc.handle_fetch(cx);
                }
            })
            .detach();
        });

        // 3. 抓取完成回调：关闭 Dialog + 处理结果
        let this_weak2 = this_weak.clone();
        let mode = mode;
        entity.update(cx, |fc, _| {
            fc.set_on_complete(Box::new(move |lits, window, cx| {
                use gpui_component::WindowExt;
                debug!("FETCH_DEBUG: on_complete 触发, 即将 close_dialog, lits.len={}", lits.len());
                window.close_dialog(cx);

                if let Some(this) = this_weak2.upgrade() {
                    this.update(cx, |this, cx| {
                        let should_select = lits.len() > 1
                            && (mode == FetchMode::Dblp || mode == FetchMode::OpenAlex);
                        debug!(
                            "FETCH_DEBUG: on_complete 处理中, lits.len={}, mode={:?}, should_select={}, pending_imports.len={}, pending_selectors.len={}",
                            lits.len(), mode, should_select, this.pending_imports.len(), this.pending_selectors.len(),
                        );

                        if should_select {
                            this.pending_selectors.push((
                                lits.into_iter().map(Arc::new).collect(),
                                Box::new(|this, lit: Literature, _window, _cx| {
                                    this.pending_imports.push(lit);
                                }),
                            ));
                            this.process_next_pending_selector(cx);
                        } else {
                            this.pending_imports.extend(lits);
                            this.process_next_pending_import(cx);
                        }
                        this.fetch_dialog = None;
                    });
                }
            }));
        });

        // 4. 打开 Dialog
        let mode_text = match mode {
            FetchMode::Doi => "DOI",
            FetchMode::ArXiv => "ArXiv",
            FetchMode::BibTeX => "BibTeX",
            FetchMode::Dblp => "DBLP",
            FetchMode::OpenAlex => "OpenAlex",
        };
        let lang = app.current_language();
        let title = tf(I18nKey::FetchFromSource, lang, &[mode_text]);

        window.open_dialog(cx, move |dialog, _, _cx| {
            dialog
                .w(px(500.))
                .title(title.clone())
                .content({
                    let this_weak = this_weak.clone();
                    move |content, _, cx| {
                        let entity = this_weak
                            .upgrade()
                            .and_then(|this| this.read(cx).fetch_dialog.clone());
                        if let Some(entity) = entity {
                            content.child(entity.clone())
                        } else {
                            content
                        }
                    }
                })
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(true)
                        .ok_text(t(I18nKey::ConfirmFetch, app.current_language()))
                        .on_ok({
                            let this_weak = this_weak.clone();
                            move |_, _, cx| {
                                if let Some(this) = this_weak.upgrade() {
                                    this.update(cx, |this, cx| {
                                        if let Some(entity) = &this.fetch_dialog {
                                            entity.update(cx, |fc, cx| fc.handle_fetch(cx));
                                        }
                                    });
                                }
                                false
                            }
                        })
                        .on_cancel({
                            let this_weak = this_weak.clone();
                            move |_, _, cx| {
                                if let Some(this) = this_weak.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.fetch_dialog = None;
                                        cx.notify();
                                        cx.notify();
                                    });
                                }
                                true
                            }
                        }),
                )
        });

        self.fetch_dialog = Some(entity.clone());
        cx.notify();

        window.defer(cx, {
            let entity = entity.clone();
            move |window, cx| {
                entity.update(cx, |this, cx| {
                    this.input_entity().update(cx, |state, cx| {
                        state.focus(window, cx);
                    });
                });
            }
        });
    }

    pub(super) fn start_fetch_and_compare(
        &mut self,
        lit: Arc<Literature>,
        source: FetchSource,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();

        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                // 将可能调用网络请求的异步代码交由全局 Tokio Runtime (RUNTIME) 执行，防止 DNS 解析器找不到 Reactor 导致崩溃
                let fetch_res = crate::RUNTIME
                    .spawn(async move { app.fetch_metadata_from_source(source).await })
                    .await;

                match fetch_res {
                    Ok(Ok(fetched)) => {
                        if let Some(this) = this_weak.upgrade() {
                            this.update(&mut cx, |this, cx| {
                                this.show_literature_compare(lit, fetched, cx);
                            });
                        }
                    }
                    Ok(Err(e)) => {
                        error!("元数据获取失败: {e}");
                    }
                    Err(e) => {
                        error!("Tokio 任务运行失败: {e}");
                    }
                }
            }
        })
        .detach();
    }

    fn process_next_pending_selector(&mut self, cx: &mut Context<Self>) {
        if self.pending_selectors.is_empty() {
            return;
        }
        info!(
            "FETCH_DEBUG: process_next_pending_selector, 剩余 {} 个",
            self.pending_selectors.len(),
        );
        let (candidates, on_select) = self.pending_selectors.remove(0);
        self.open_metadata_selector(candidates, cx, on_select);
    }

    fn process_next_pending_import(&mut self, cx: &mut Context<Self>) {
        if self.pending_imports.is_empty() {
            return;
        }

        let lit = self.pending_imports.remove(0);
        info!(
            "UI: 处理批量导入队列，剩余 {} 条，正在打开编辑器: {} (active_popup_count={})",
            self.pending_imports.len(),
            lit.title,
            self.active_popup_count,
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
            self.show_literature_editor((*lit).clone(), false, cx);
        }
    }

    fn confirm_add_literature(&mut self, lit: Literature, cx: &mut Context<Self>) {
        let title = lit.title.clone();
        let lit_id = lit.id.clone();
        info!("业务: 用户确认添加新文献: {title}");

        match self.app.add_literature(lit) {
            Ok(()) => {
                info!("成功添加文献: {title}");
                // 选中新添加的文献
                crate::services::ui_state::UiState::update(cx, |state| {
                    state.selected_literature_ids.clear();
                    state.selected_literature_ids.insert(lit_id.clone());
                });
                // 如果当前选中的是某个自定义文件夹，自动将新文献加入此文件夹
                let state = cx.global::<crate::services::ui_state::UiState>();
                if let Some(ref folder_id) = state.selected_folder_id {
                    let virtual_folders =
                        ["all", "trash", "unread", "reading", "read", "favorites"];
                    if !virtual_folders.contains(&folder_id.as_str()) {
                        info!(
                            "业务: 自动将新文献[{}]加入当前选中文件夹[{}]",
                            lit_id, folder_id
                        );
                        if let Err(e) = self.app.add_literature_to_folder(&lit_id, folder_id) {
                            error!("自动关联文件夹失败: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                error!("添加文献失败: {e}");
                show_notification(NotificationType::Error, format!("添加文献失败: {e}"), cx);
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

    pub fn run_duplicate_detection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let groups = self.app.find_duplicates();
        let lang = self.app.current_language();

        if groups.is_empty() {
            show_notification(
                NotificationType::Info,
                format!(
                    "{}: {}",
                    t(I18nKey::DuplicateGroups, lang),
                    t(I18nKey::NoDuplicatesFound, lang)
                ),
                cx,
            );
            return;
        }

        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let groups_clone = groups.clone();

        let entity = cx.new(|_| DuplicateListDialogContent::new(app.clone(), groups, false));
        let entity_weak = entity.downgrade();
        entity.update(cx, |dc, _| {
            dc.set_on_complete(Box::new(move |idx, w, cx| {
                w.close_dialog(cx);
                if let Some(this) = this_weak.upgrade() {
                    this.update(cx, |this, cx| {
                        if let Some(idx) = idx {
                            let group = groups_clone[idx].clone();
                            this.start_merge_flow(group, cx);
                        }
                        cx.notify();
                    });
                }
            }));
        });
        self.duplicate_dialog = Some(entity.clone());

        window.open_dialog(cx, move |dialog, _, _cx| {
            let entity_weak_content = entity_weak.clone();
            dialog
                .w(px(600.))
                .title(t(I18nKey::DuplicateGroups, app.current_language()))
                .content(move |content, _, _cx| {
                    if let Some(e) = entity_weak_content.upgrade() {
                        content.child(e)
                    } else {
                        content
                    }
                })
        });
    }

    pub fn handle_sync_conflicts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let groups = if let Ok(state) = self.app.sync_state.lock() {
            state.sync_conflict_groups.clone()
        } else {
            None
        };

        if let Some(groups) = groups {
            let app = self.app.clone();
            let this_weak = cx.entity().downgrade();
            let groups_clone = groups.clone();

            let entity = cx.new(|_| DuplicateListDialogContent::new(app.clone(), groups, true));
            let entity_weak = entity.downgrade();
            entity.update(cx, |dc, _| {
                dc.set_on_complete(Box::new(move |idx, w, cx| {
                    w.close_dialog(cx);
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            if let Some(idx) = idx {
                                let group = groups_clone[idx].clone();
                                this.start_sync_conflict_resolve_flow(group, w, cx);
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
                }));
            });
            self.duplicate_dialog = Some(entity.clone());

            window.open_dialog(cx, move |dialog, _, _cx| {
                let entity_weak_content = entity_weak.clone();
                dialog
                    .w(px(600.))
                    .title(t(I18nKey::SyncConflicts, app.current_language()))
                    .content(move |content, _, _cx| {
                        if let Some(e) = entity_weak_content.upgrade() {
                            content.child(e)
                        } else {
                            content
                        }
                    })
            });
        }
    }

    fn start_sync_conflict_resolve_flow(
        &mut self,
        group: Vec<Literature>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if group.len() < 2 {
            return;
        }

        let local_lit = Arc::new(group[0].clone());
        let remote_lit = group[1].clone();
        self.resolve_next_sync_conflict(local_lit, remote_lit, window, cx);
    }

    fn open_modal_window<V: Render>(
        &mut self,
        size: Size<Pixels>,
        cx: &mut Context<Self>,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V + Send + 'static,
    ) {
        debug!(
            "MODAL_DEBUG: open_modal_window 入口, active_popup_count={}, size={:?}",
            self.active_popup_count, size,
        );
        if self.active_popup_count > 0 {
            debug!("MODAL: 已有活跃弹窗，跳过 (size={:?})", size);
            return;
        }
        let bounds = Bounds::centered(None, size, cx);
        debug!("MODAL: 开始创建窗口 (size={:?}, bounds={:?})", size, bounds);

        self.active_popup_count += 1;
        debug!(
            "MODAL_DEBUG: active_popup_count 增至 {}",
            self.active_popup_count
        );
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
                            debug!(
                                "MODAL_DEBUG: active_popup_count 降至 {} (after release)",
                                this.active_popup_count
                            );
                            if this.active_popup_count == 0 {
                                if !this.pending_selectors.is_empty() {
                                    this.process_next_pending_selector(cx);
                                } else {
                                    this.process_next_pending_import(cx);
                                }
                            }
                            cx.notify();
                        });
                    }
                })
                .detach();
                // Windows 下程序化打开的窗口默认不会获得前台焦点，会落到主窗口后面，
                // 这里显式将其激活到前台。defer 确保窗口已创建并显示后再激活。
                window.defer(cx, |window, _cx| {
                    window.activate_window();
                });
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
        local_lit: Arc<Literature>,
        remote_lit: Literature,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let main_window_handle = window.window_handle();

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
                                let mut local_fixed = (*local_lit).clone();
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
                                let this_weak_reopen = this_weak_cb.clone();
                                let _ = cx.update_window(main_window_handle, |_, window, cx| {
                                    if let Some(this) = this_weak_reopen.upgrade() {
                                        this.update(cx, |this, cx| {
                                            this.handle_sync_conflicts(window, cx);
                                        });
                                    }
                                });
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

        // 智能推荐最佳主文件（附件多、元数据完备的优先）
        let best = group
            .iter()
            .enumerate()
            .max_by_key(|(_, lit)| {
                let pub_name = lit
                    .publication
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("");
                let meta_score = if lit.title.is_empty() { 0 } else { 1 }
                    + if lit.doi.is_some() { 1 } else { 0 }
                    + if !pub_name.is_empty() { 1 } else { 0 };
                lit.attachments.len() * 10 + meta_score
            })
            .map(|(i, _)| i)
            .unwrap_or(0);
        if best != 0 {
            group.swap(0, best);
        }

        let original = group.remove(0);
        self.merge_next_in_group(Arc::new(original), group, cx);
    }

    fn merge_next_in_group(
        &mut self,
        original: Arc<Literature>,
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
            show_notification(
                NotificationType::Success,
                format!(
                    "{}: {}",
                    t(I18nKey::LiteratureMergedTitle, lang),
                    tf(I18nKey::LiteratureMergedMsg, lang, &[&next_lit.title])
                ),
                cx,
            );

            self.continue_merge_flow(original_clone, remaining_clone, cx);
            return;
        }

        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();

        let diff = FieldSelection::compare(&original, &next_lit);
        let size = size(px(1100.0), px(750.0));

        self.open_modal_window(size, cx, move |_window, _cx| {
            let this_weak_cb = this_weak.clone();
            let app_cb = app.clone();
            let remaining_cb = remaining.clone();
            let original_cb = original.clone();
            let next_cb = next_lit.clone();
            let diff_cb = diff.clone();

            crate::ui::components::MergeDialog::new(
                app.clone(),
                (*original_cb).clone(),
                next_cb.clone(),
                diff_cb,
                Box::new(move |result, window, cx| {
                    if let Some(this) = this_weak_cb.upgrade() {
                        if let Some(res) = result {
                            let master_id = &res.master_id;
                            let source_id = &res.source_id;
                            let sel = &res.selection;

                            let (master_lit, source_lit) = if master_id == &original_cb.id {
                                (original_cb.as_ref(), &next_cb)
                            } else {
                                (&next_cb, original_cb.as_ref())
                            };

                            // 应用字段选择
                            let mut merged = master_lit.clone();
                            if sel.literature_type {
                                merged.literature_type = source_lit.literature_type.clone();
                            }
                            if sel.title {
                                merged.title = source_lit.title.clone();
                            }
                            if sel.authors {
                                merged.authors = source_lit.authors.clone();
                            }
                            if sel.year {
                                merged.year = source_lit.year;
                            }
                            if sel.month {
                                merged.month = source_lit.month;
                            }
                            if sel.day {
                                merged.day = source_lit.day;
                            }
                            if sel.journal {
                                merged.publication = source_lit.publication.clone();
                            }
                            if sel.volume {
                                merged.volume = source_lit.volume.clone();
                            }
                            if sel.issue {
                                merged.issue = source_lit.issue.clone();
                            }
                            if sel.pages {
                                merged.pages = source_lit.pages.clone();
                            }
                            if sel.publisher {
                                if let Some(ref pub_src) = source_lit.publication {
                                    if let Some(ref p) = merged.publication {
                                        let mut p2 = p.clone();
                                        p2.publisher = pub_src.publisher.clone();
                                        merged.publication = Some(p2);
                                    } else {
                                        merged.publication = Some(pub_src.clone());
                                    }
                                }
                            }
                            if sel.abstract_text {
                                merged.abstract_text = source_lit.abstract_text.clone();
                            }
                            if sel.doi {
                                merged.doi = source_lit.doi.clone();
                            }
                            if sel.arxiv_id {
                                merged.arxiv_id = source_lit.arxiv_id.clone();
                            }
                            if sel.url {
                                merged.url = source_lit.url.clone();
                            }

                            info!("合并流程: 确认合并。主文件={}, 源={}", master_id, source_id);

                            let (a_main, _a_others) = {
                                let mut main_att = None;
                                let mut others = Vec::new();
                                if let Some(pos) =
                                    original_cb.attachments.iter().position(|a| a.is_main)
                                {
                                    main_att = Some(original_cb.attachments[pos].clone());
                                    for (i, att) in original_cb.attachments.iter().enumerate() {
                                        if i != pos {
                                            others.push(att.clone());
                                        }
                                    }
                                } else if let Some(first) = original_cb.attachments.first() {
                                    main_att = Some(first.clone());
                                    others = original_cb.attachments[1..].to_vec();
                                }
                                (main_att, others)
                            };

                            let (b_main, _b_others) = {
                                let mut main_att = None;
                                let mut others = Vec::new();
                                if let Some(pos) =
                                    next_cb.attachments.iter().position(|a| a.is_main)
                                {
                                    main_att = Some(next_cb.attachments[pos].clone());
                                    for (i, att) in next_cb.attachments.iter().enumerate() {
                                        if i != pos {
                                            others.push(att.clone());
                                        }
                                    }
                                } else if let Some(first) = next_cb.attachments.first() {
                                    main_att = Some(first.clone());
                                    others = next_cb.attachments[1..].to_vec();
                                }
                                (main_att, others)
                            };

                            // 统一处理所有附件：若非选中的主PDF且未在保留列表中，则删除
                            let mut all_atts = Vec::new();
                            all_atts.extend(original_cb.attachments.clone());
                            all_atts.extend(next_cb.attachments.clone());

                            for att in all_atts {
                                let is_chosen_main = (Some(att.id.clone())
                                    == a_main.as_ref().map(|x| x.id.clone())
                                    && res.keep_a_main_pdf)
                                    || (Some(att.id.clone())
                                        == b_main.as_ref().map(|x| x.id.clone())
                                        && res.keep_b_main_pdf);

                                if !is_chosen_main && !res.keep_attachment_ids.contains(&att.id) {
                                    if let Err(e) = app_cb.delete_attachment_file(&att.id) {
                                        error!("合并流程: 删除未保留的附件失败: {e}");
                                    }
                                }
                            }

                            if let Err(e) = app_cb.update_literature(merged.clone()) {
                                error!("合并流程: 保存合并结果失败: {e}");
                            }

                            if let Err(e) = app_cb.merge_literature_relations(source_id, master_id)
                            {
                                error!("合并流程: 合并关联关系失败: {e}");
                            }

                            if let Err(e) = app_cb.delete_literature_by_id(source_id) {
                                error!("合并流程: 移动副本到回收站失败: {e}");
                            }

                            let lang = app_cb.current_language();
                            show_notification(
                                NotificationType::Success,
                                format!(
                                    "{}: {}",
                                    t(I18nKey::LiteratureMergedTitle, lang),
                                    tf(I18nKey::LiteratureMergedMsg, lang, &[&source_lit.title])
                                ),
                                cx,
                            );

                            this.update(cx, |this, cx| {
                                this.continue_merge_flow(
                                    Arc::new(merged),
                                    remaining_cb.clone(),
                                    cx,
                                );
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
                }),
            )
        });
    }

    fn continue_merge_flow(
        &mut self,
        original: Arc<Literature>,
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
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(150))
                    .await;
                cx.update(|cx| {
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
