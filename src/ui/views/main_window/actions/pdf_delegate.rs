use ai::ChatRole;
use i18n::Language;
use log::{debug, error};
use services::library::PdfPersistence;
use services::pdf::{AiBackendItem, PdfInitialState, PdfReaderDelegate};
use std::sync::Arc;
pub struct AppPdfDelegate {
    pub(crate) app: Arc<services::app::MainApp>,
    pub(crate) literature_id: String,
    pub(crate) pdf: PdfPersistence,
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

    fn set_translation_engine(&self, name: String) {
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
        self.pdf.load_annotations(&self.app.db, id)
    }

    fn save_annotation(&self, annotation: &models::Annotation) {
        self.pdf.save_annotation(&self.app.db, annotation);
    }

    fn delete_annotation(&self, id: &str) {
        self.pdf.delete_annotation(&self.app.db, id);
    }

    fn on_link_click(&self, url: String) {
        crate::ui::views::main_window::utils::open_url(&url);
    }

    fn list_notes(&self, literature_id: &str) -> Vec<models::LiteratureNote> {
        self.app
            .literature_service
            .list_notes(&self.app.db, literature_id)
    }

    fn create_note(&self, literature_id: &str, title: &str) -> Option<String> {
        let id = self
            .app
            .literature_service
            .create_note(&self.app.db, literature_id, title)?;
        self.app.notify_data_changed();
        Some(id)
    }

    fn update_note(&self, note_id: &str, title: Option<&str>, content: Option<&str>) -> bool {
        let ok = self
            .app
            .literature_service
            .update_note(&self.app.db, note_id, title, content);
        if ok {
            self.app.notify_data_changed();
        }
        ok
    }

    fn delete_note(&self, note_id: &str) -> bool {
        let ok = self
            .app
            .literature_service
            .delete_note(&self.app.db, note_id);
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
            .attachment_service
            .literature_attachments(&self.app.db, &self.literature_id)
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
                                    services::pdf::extract_text_from_pdf(fp).ok()
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

    fn set_active_chat_backend(&self, name: String) {
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
