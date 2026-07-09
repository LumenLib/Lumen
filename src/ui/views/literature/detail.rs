use crate::notification_bus::show_notification;
use crate::services::MainApp;
use crate::services::data_store::DataStore;
use crate::ui::{
    components::{CollapsibleText, DetailRow, LinkRow, muted_input, render_icon_button},
    icons::IconName,
    views::main_window::{self, ContextMenuType, MainWindow},
};
use futures_util::{StreamExt, TryFutureExt};
use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, AsyncApp, ClickEvent, DragMoveEvent, Entity, ExternalPaths, FontWeight,
    MouseButton, SharedString, Task, WeakEntity, Window, div, rems,
};
use gpui_component::{
    ActiveTheme, Colorize, Icon, Theme,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    v_flex,
};
use i18n::{I18nKey, Language, t, tf};
use log::{debug, error, info};
use models::{Literature, ReadingStatus};
use parser::normalize::author_full_name;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// 文献详情视图的预实体化状态 (Buffer)
#[derive(Clone)]
struct DetailState {
    /// 当前选中的 ID 列表（用于变更检测）
    selected_ids: Vec<String>,
    /// 当前缓冲文献的版本号（用于检测内容变更）
    content_version: i32,
    /// 渲染模式
    mode: DetailMode,
}

#[derive(Clone)]
struct TagData {
    name: String,
    color: String,
}

#[derive(Clone)]
enum DetailMode {
    /// 无选中
    None,
    /// 选中多个
    Multiple(usize),
    /// 选中单个并预处理好数据
    Single(Box<SingleDetailBuffer>),
}

/// 单个文献的渲染缓冲数据
#[derive(Clone)]
struct SingleDetailBuffer {
    literature: Arc<Literature>,
    ccf_badge: Option<BadgeData>,
    jcr_badge: Option<BadgeData>,
    cas_badge: Option<BadgeData>,
    authors_text: String,
    pub_name: String,
    abstract_display: String,
    rating: i32,
    tags: Vec<TagData>,
    references: Vec<Arc<Literature>>,
    cited_by: Vec<Arc<Literature>>,
    reading_status: ReadingStatus,
    folder_paths: Vec<Vec<String>>,
}

#[derive(Clone)]
struct BadgeData {
    text: String,
    bg: gpui::Hsla,
    fg: gpui::Hsla,
}

/// 右侧文献详情视图
pub struct LiteratureDetailView {
    /// 应用控制器
    app: Arc<MainApp>,
    /// 数据存储实体
    pub data_store: Entity<DataStore>,
    /// 是否正在拖入文件
    is_dragging: bool,
    /// 摘要是否展开
    abstract_expanded: bool,
    /// 标签是否展开
    tags_expanded: bool,
    /// 文件夹是否展开
    folders_expanded: bool,
    /// 关联文献是否展开
    citations_expanded: bool,
    /// 笔记是否展开
    notes_expanded: bool,
    /// 多笔记卡片
    notes_cache: Vec<models::LiteratureNote>,
    editing_note_index: Option<usize>,
    edit_note_title: Option<Entity<InputState>>,
    edit_note_content: Option<Entity<InputState>>,
    /// AI 总结任务句柄
    summary_task: Option<Task<()>>,
    /// 是否正在生成 AI 总结
    is_generating_summary: bool,
    /// 上一次 AI 总结的笔记 ID（用于替换）
    last_ai_summary_note_id: Option<String>,
    /// 父视图句柄 (`MainWindow`)
    parent_view: Option<WeakEntity<MainWindow>>,
    /// 预实体化缓冲状态
    state: DetailState,
    /// 鼠标当前悬停的评分值（用于预览）
    hovered_rating: i32,
    /// Copy feedback state
    copied_field: Option<String>,
    /// 展开的单个笔记 ID 集合
    expanded_notes: std::collections::HashSet<String>,
}

impl LiteratureDetailView {
    pub fn new(app: Arc<MainApp>, data_store: Entity<DataStore>) -> Self {
        debug!("文献详情: 初始化");
        Self {
            app,
            data_store,
            is_dragging: false,
            abstract_expanded: false,
            tags_expanded: false,
            folders_expanded: false,
            citations_expanded: false,
            notes_expanded: false,
            notes_cache: Vec::new(),
            editing_note_index: None,
            edit_note_title: None,
            edit_note_content: None,
            summary_task: None,
            is_generating_summary: false,
            last_ai_summary_note_id: None,
            parent_view: None,
            state: DetailState {
                selected_ids: Vec::new(),
                content_version: -1,
                mode: DetailMode::None,
            },
            hovered_rating: 0,
            copied_field: None,
            expanded_notes: std::collections::HashSet::new(),
        }
    }

    pub fn reload_notes(&mut self, cx: &mut Context<Self>) {
        if let Some(lit_id) = self.state.selected_ids.first()
            && let Ok(notes) = self.app.db.list_notes(lit_id)
        {
            let has_generating = self.is_generating_summary;
            let mut merged_notes = notes;
            if has_generating
                && let Some(gen_node) = self
                    .notes_cache
                    .iter()
                    .find(|n| n.id == "ai_generating_note")
                    .cloned()
            {
                merged_notes.push(gen_node);
            }
            self.notes_cache = merged_notes;
        }
        cx.notify();
    }

    fn generate_ai_summary(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let lit_id = match self.state.selected_ids.first() {
            Some(id) => id.clone(),
            None => return,
        };

        // 删除上一次的 AI 总结
        if let Some(last_id) = self.last_ai_summary_note_id.take() {
            let _ = self.app.db.delete_note(&last_id);
            self.notes_cache.retain(|n| n.id != last_id);
        }

        self.notes_cache.retain(|n| n.id != "ai_generating_note");

        let now = chrono::Utc::now().timestamp();
        self.notes_cache.push(models::LiteratureNote {
            id: "ai_generating_note".to_string(),
            literature_id: lit_id.clone(),
            title: "AI 总结生成中...".to_string(),
            content: "正在准备数据，请稍候...\n\n".to_string(),
            sort_order: self.notes_cache.len() as i32,
            created_at: now,
            updated_at: now,
        });

        self.is_generating_summary = true;
        self.notes_expanded = true;
        cx.notify();

        let app = self.app.clone();
        let lit_id_clone = lit_id.clone();

        let task = cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                let result: Result<String, String> = async {
                    let lit = app
                        .db
                        .get_literature(&lit_id_clone)
                        .map_err(|e| format!("读取文献数据库失败: {:?}", e))?
                        .ok_or_else(|| "未找到指定文献".to_string())?;

                    let mut pdf_path = None;
                    for att in &lit.attachments {
                        if att.file_path.to_lowercase().ends_with(".pdf") {
                            pdf_path = Some(att.file_path.clone());
                            break;
                        }
                    }

                    let mut pdf_text = None;
                    if let Some(path) = pdf_path {
                        let _ = this.update(&mut cx, |this, cx| {
                            if let Some(n) = this.notes_cache.iter_mut().find(|n| n.id == "ai_generating_note") {
                                n.content = "正在提取 PDF 纯文本，这可能需要一点时间...\n\n".to_string();
                            }
                            cx.notify();
                        });

                        pdf_text = Some(pdf::extract_text_from_pdf(&path).map_err(|e| format!("PDF 文本解析失败: {:?}", e))?);
                    }

                    let _ = this.update(&mut cx, |this, cx| {
                        if let Some(n) = this.notes_cache.iter_mut().find(|n| n.id == "ai_generating_note") {
                            n.content = "正在发起 AI 总结生成...\n\n".to_string();
                        }
                        cx.notify();
                    });

                    let keys = app.local_state.read().unwrap().translation_keys.clone();
                    let entries_json = keys.get("ai.entries").cloned().unwrap_or_default();
                    let active_name = keys.get("chat.active").cloned().unwrap_or_default();
                    let entries: Vec<ai::AiBackendEntry> =
                        serde_json::from_str(&entries_json).unwrap_or_default();
                    let entry = entries
                        .iter()
                        .find(|e| e.name == active_name)
                        .ok_or_else(|| "未配置默认 AI 聊天模型，请在设置中配置".to_string())?;

                    let kind = ai::BackendKind::from_str(&entry.kind);
                    let config = entry.to_config();
                    let service = ai::AiService::new(kind, &config);

                    let mut prompt_content = format!(
                        "文献标题: {}\n摘要: {}\n",
                        lit.title,
                        lit.abstract_text.as_deref().unwrap_or("")
                    );
                    if let Some(text) = pdf_text {
                        prompt_content.push_str(&format!("\n正文全文:\n{}", text));
                    }

                    let messages = vec![
                        ai::ChatMessage {
                            role: ai::ChatRole::User,
                            content: prompt_content,
                            attachments: Vec::new(),
                        }
                    ];

                    let system_prompt = "你是一个精通学术论文分析的 AI 助手。请针对用户给出的文献（包含标题、摘要及提取的全文），写一份详细且条理清晰的学术总结。总结必须包含：1. 研究背景与动机（作者为什么要研究这个问题）；2. 核心方法与模型（作者是如何实现和解决这个问题的，包含哪些技术核心）；3. 关键实验结果（核心数据、结论等）；4. 主要结论与学术贡献。请用中文回答，并以清晰易读的 Markdown 格式输出。注意：必须直接输出 Markdown 纯文本，严禁在最外层使用 ```markdown ... ``` 或 ``` ... ``` 这样的代码块标记包裹整篇回答。所有数学符号、希腊字母、公式等使用 LaTeX 语法书写，公式必须且只能使用 $$ 包裹（例如 $$a^2 + b^2 = c^2$$，不要使用 \\(...\\) 或 \\[...\\] 等包裹方式），同时请避免输出复杂或多行的公式，尽量使用简单、单行的公式形式。";

                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                    let result_handle = crate::RUNTIME.spawn(async move {
                        let mut stream = service
                            .chat_stream(&messages, Some(system_prompt))
                            .map_err(|e| format!("AI 服务请求失败: {:?}", e))
                            .await?;

                        while let Some(chunk) = stream.next().await {
                            let chunk_text = chunk.map_err(|e| format!("流传输异常: {:?}", e))?;
                            match &chunk_text {
                                models::chat::ChatResponseChunk::Content(text) => {
                                    log::info!(
                                        "[AI Summary Chunk(detail)] Content: len={}, preview={:?}",
                                        text.len(),
                                        &text[..text.len().min(80)]
                                    );
                                    let _ = tx.send(text.clone());
                                }
                                other => {
                                    log::info!("[AI Summary Chunk(detail)] Other variant: {:?}", other);
                                }
                            }
                        }
                        Ok::<(), String>(())
                    });

                    let mut full_output = String::new();
                    while let Some(text) = rx.recv().await {
                        full_output.push_str(&text);
                        let display_output = full_output.clone();
                        let _ = this.update(&mut cx, |this, cx| {
                            if let Some(n) = this.notes_cache.iter_mut().find(|n| n.id == "ai_generating_note") {
                                n.content = display_output;
                            }
                            cx.notify();
                        });
                    }

                    match result_handle.await {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => return Err(err),
                        Err(e) => return Err(format!("Tokio 任务执行异常: {:?}", e)),
                    }

                    if full_output.trim().is_empty() {
                        return Err("AI 服务返回了空内容".to_string());
                    }

                    log::info!(
                        "[AI Summary Final(detail)] total_len={}, starts_with_code_block={}, ends_with_code_block={}, preview_end={:?}",
                        full_output.len(),
                        full_output.trim().starts_with("```"),
                        full_output.trim().ends_with("```"),
                        full_output.chars().rev().take(200).collect::<String>()
                    );

                    let note_id = app
                        .db
                        .create_note(&lit_id_clone, "AI 总结")
                        .map_err(|e| format!("创建文献笔记失败: {:?}", e))?;

                    let _ = this.update(&mut cx, |this, _cx| {
                        this.last_ai_summary_note_id = Some(note_id.clone());
                    });

                    let ok = app
                        .db
                        .update_note(&note_id, Some("AI 总结"), Some(&full_output))
                        .unwrap_or(false);

                    if !ok {
                        return Err("保存笔记内容失败".to_string());
                    }

                    app.notify_data_changed();

                    Ok(full_output)
                }.await;

                let _ = this.update(&mut cx, |this, cx| {
                    this.is_generating_summary = false;
                    this.notes_cache.retain(|n| n.id != "ai_generating_note");
                    match result {
                        Ok(_) => {
                            this.reload_notes(cx);
                        }
                        Err(err_msg) => {
                            error!("AI 总结生成失败: {}", err_msg);
                            show_notification(
                                NotificationType::Error,
                                format!("AI 总结生成失败: {}", err_msg),
                                cx,
                            );
                            this.reload_notes(cx);
                        }
                    }
                    cx.notify();
                });
            }
        });

        self.summary_task = Some(task);
    }

    pub fn set_parent_view(&mut self, parent: WeakEntity<MainWindow>) {
        self.parent_view = Some(parent);
    }

    fn sync_state(&mut self, cx: &mut Context<Self>) {
        if !self.sync_detect_changes(cx) {
            return;
        }
        self.sync_update_mode(cx);
        cx.notify();
    }

    fn sync_detect_changes(&mut self, cx: &Context<Self>) -> bool {
        let ui = cx.global::<crate::services::ui_state::UiState>();
        let store = self.data_store.read(cx);
        let current_selected: Vec<String> = ui.selected_literature_ids.iter().cloned().collect();
        let selected_count = current_selected.len();

        let ids_changed = self.state.selected_ids != current_selected;

        let version_changed = if selected_count == 1 {
            current_selected
                .first()
                .and_then(|id| store.literatures.iter().find(|l| l.id == *id))
                .is_none_or(|lit| lit.version != self.state.content_version)
        } else {
            false
        };

        let tags_changed = if let DetailMode::Single(ref buffer) = self.state.mode {
            buffer.tags.iter().any(|tag_data| {
                store
                    .tags
                    .iter()
                    .find(|(t, _)| t.name == tag_data.name)
                    .is_none_or(|(t, _)| t.color != tag_data.color)
            })
        } else {
            false
        };

        if !ids_changed && !version_changed && !tags_changed {
            return false;
        }

        debug!(
            "详情: 检测到变化 (ids={ids_changed}, version={version_changed}, tags={tags_changed})"
        );
        self.state.selected_ids = current_selected;
        true
    }

    fn sync_update_mode(&mut self, cx: &Context<Self>) {
        let selected_count = self.state.selected_ids.len();
        if selected_count == 0 {
            self.state.mode = DetailMode::None;
            self.state.content_version = -1;
        } else if selected_count > 1 {
            self.state.mode = DetailMode::Multiple(selected_count);
            self.state.content_version = -1;
        } else if let Some(buffer) = self.sync_build_buffer(cx) {
            self.state.content_version = buffer.literature.version;
            self.state.mode = DetailMode::Single(Box::new(buffer));
            let lit_id = &self.state.selected_ids[0];
            if let Ok(notes) = self.app.db.list_notes(lit_id) {
                self.notes_cache = notes;
            }
        } else {
            self.state.mode = DetailMode::None;
        }
        debug!("详情: 模式切换 -> {} 个选中", selected_count);
    }

    fn sync_build_buffer(&self, cx: &Context<Self>) -> Option<SingleDetailBuffer> {
        let store = self.data_store.read(cx);
        let theme = cx.theme().clone();
        let first_id = self.state.selected_ids.first()?;
        let lit = store
            .literatures
            .iter()
            .find(|l| l.id == *first_id)
            .cloned()?;

        let authors_text = lit
            .authors
            .iter()
            .map(author_full_name)
            .collect::<Vec<_>>()
            .join(", ");

        let pub_name = lit
            .publication
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let jcr_badge = Self::build_jcr_badge(&lit, &theme);
        let ccf_badge = Self::build_ccf_badge(&lit, &theme);
        let cas_badge = Self::build_cas_badge(&lit, &theme);
        let abstract_display = self.build_abstract_display(&lit);
        let tags = Self::build_tags(&lit, store);
        let references = self.build_references(&lit, store);
        let cited_by = self.build_cited_by(&lit, store);
        let folder_paths = Self::build_folder_paths(&lit, store, self.app.current_language());

        debug!(
            "详情: 构建缓冲完毕 (title='{}', authors={}, tags={}, refs={}, cited={})",
            lit.title,
            lit.authors.len(),
            lit.tags.len(),
            references.len(),
            cited_by.len()
        );

        Some(SingleDetailBuffer {
            literature: lit.clone(),
            ccf_badge,
            jcr_badge,
            cas_badge,
            authors_text,
            pub_name,
            abstract_display,
            rating: lit.rating,
            tags,
            references,
            cited_by,
            reading_status: lit.reading_status,
            folder_paths,
        })
    }

    fn build_jcr_badge(lit: &Literature, theme: &Theme) -> Option<BadgeData> {
        lit.publication
            .as_ref()
            .and_then(|p| p.jcr_rank.as_ref())
            .map(|rank| {
                let (bg, fg) = match rank.as_str() {
                    "Q1" => (theme.green, theme.primary_foreground),
                    "Q2" => (theme.blue, theme.primary_foreground),
                    "Q3" => (theme.yellow, theme.primary_foreground),
                    "Q4" => (theme.red, theme.primary_foreground),
                    _ => (theme.muted, theme.muted_foreground),
                };
                BadgeData {
                    text: format!("JCR {rank}"),
                    bg,
                    fg,
                }
            })
    }

    fn build_ccf_badge(lit: &Literature, theme: &Theme) -> Option<BadgeData> {
        lit.publication
            .as_ref()
            .and_then(|p| p.ccf_rank.as_ref())
            .map(|rank| {
                let (bg, fg) = match rank.as_str() {
                    "A" => (theme.red, theme.primary_foreground),
                    "B" => (theme.yellow, theme.primary_foreground),
                    "C" => (theme.blue, theme.primary_foreground),
                    _ => (theme.muted, theme.muted_foreground),
                };
                BadgeData {
                    text: format!("CCF {rank}"),
                    bg,
                    fg,
                }
            })
    }

    fn build_cas_badge(lit: &Literature, theme: &Theme) -> Option<BadgeData> {
        lit.publication
            .as_ref()
            .and_then(|p| p.cas_rank.as_ref())
            .map(|rank| {
                let (bg, fg) = if rank.contains("1区") {
                    (theme.red, theme.primary_foreground)
                } else if rank.contains("2区") {
                    (theme.yellow, theme.primary_foreground)
                } else if rank.contains("3区") {
                    (theme.blue, theme.primary_foreground)
                } else {
                    (theme.muted, theme.muted_foreground)
                };

                let display_text = if let Some(idx) = rank.find("区") {
                    if idx > 0 {
                        let 区_idx = rank.chars().take(idx + 1).count() - 1;
                        if 区_idx > 0
                            && rank.chars().nth(区_idx - 1).is_some_and(|c| c.is_numeric())
                        {
                            format!(
                                "CAS {}{}",
                                rank.chars().nth(区_idx - 1).unwrap_or(' '),
                                rank.chars().nth(区_idx).unwrap_or(' ')
                            )
                        } else {
                            format!("CAS {rank}")
                        }
                    } else {
                        format!("CAS {rank}")
                    }
                } else {
                    format!("CAS {rank}")
                };

                BadgeData {
                    text: display_text,
                    bg,
                    fg,
                }
            })
    }

    fn build_abstract_display(&self, lit: &Literature) -> String {
        if let Some(ref text) = lit.abstract_text {
            if !self.abstract_expanded && text.chars().count() > 30 {
                let mut truncated = text.chars().take(30).collect::<String>();
                truncated.push_str("...");
                truncated
            } else {
                text.clone()
            }
        } else {
            String::new()
        }
    }

    fn build_tags(lit: &Literature, store: &DataStore) -> Vec<TagData> {
        lit.tags
            .iter()
            .map(|tag_name| {
                let color = store
                    .tags
                    .iter()
                    .find(|(t, _)| t.name == *tag_name)
                    .map_or_else(|| "#4A90E2".to_string(), |(t, _)| t.color.clone());
                TagData {
                    name: tag_name.clone(),
                    color,
                }
            })
            .collect()
    }

    fn build_references(&self, lit: &Literature, store: &DataStore) -> Vec<Arc<Literature>> {
        self.app
            .db
            .get_references(&lit.id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| {
                store
                    .literatures
                    .iter()
                    .find(|l| l.id == c.target_id)
                    .cloned()
            })
            .collect()
    }

    fn build_cited_by(&self, lit: &Literature, store: &DataStore) -> Vec<Arc<Literature>> {
        self.app
            .db
            .get_cited_by(&lit.id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| {
                store
                    .literatures
                    .iter()
                    .find(|l| l.id == c.source_id)
                    .cloned()
            })
            .collect()
    }

    fn build_folder_paths(lit: &Literature, store: &DataStore, lang: Language) -> Vec<Vec<String>> {
        lit.folder_ids
            .iter()
            .map(|folder_id| {
                let mut path = Vec::new();
                let mut current_id = Some(folder_id.clone());
                while let Some(id) = current_id {
                    if let Some(folder) = store.folders.iter().find(|f| f.id == id) {
                        path.push(folder.name.clone());
                        current_id = folder.parent_id.clone();
                    } else {
                        let name = match id.as_str() {
                            "all" => t(I18nKey::AllLiterature, lang),
                            "uncategorized" => t(I18nKey::Uncategorized, lang),
                            "trash" => t(I18nKey::Trash, lang),
                            _ => &id,
                        };
                        path.push(name.to_string());
                        current_id = None;
                    }
                }
                path.reverse();
                path
            })
            .collect()
    }

    fn toggle_abstract(&mut self, cx: &mut Context<Self>) {
        debug!("详情: 切换摘要展开={}", !self.abstract_expanded);
        self.abstract_expanded = !self.abstract_expanded;
        if let DetailMode::Single(ref mut buffer) = self.state.mode
            && let Some(ref text) = buffer.literature.abstract_text
        {
            buffer.abstract_display = if !self.abstract_expanded && text.chars().count() > 30 {
                let mut truncated = text.chars().take(30).collect::<String>();
                truncated.push_str("...");
                truncated
            } else {
                text.clone()
            };
        }
        cx.notify();
    }

    fn copy_text(
        &mut self,
        text: String,
        field_id: String,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        info!("详情: 复制字段 '{}'", field_id);
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.copied_field = Some(field_id);
        cx.notify();

        cx.spawn(move |view: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                let _ = cx.update_window(window, |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.copied_field = None;
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }

    // =========================================================================
    // Rendering helpers
    // =========================================================================

    fn render_folder_paths(
        &self,
        buffer: &SingleDetailBuffer,
        theme: &Theme,
        lang: Language,
    ) -> impl IntoElement {
        let folder_paths = buffer.folder_paths.clone();
        let list: Vec<Vec<String>> = if folder_paths.is_empty() {
            vec![vec![t(I18nKey::Uncategorized, lang).to_string()]]
        } else {
            folder_paths
        };

        v_flex()
            .gap_1()
            .px_5()
            .children(list.into_iter().enumerate().map(|(idx, path)| {
                let path_len = path.len();
                h_flex()
                    .id(("folder-path", idx))
                    .gap_1()
                    .items_center()
                    .child(
                        Icon::new(IconName::Folder)
                            .size(rems(0.75))
                            .text_color(theme.muted_foreground),
                    )
                    .child(h_flex().flex_wrap().items_center().children(
                        path.into_iter().enumerate().map(|(p_idx, name)| {
                            h_flex()
                                .items_center()
                                .child(div().text_xs().text_color(theme.foreground).child(name))
                                .when(p_idx < path_len - 1, |this| {
                                    this.child(
                                        Icon::new(IconName::ChevronRight)
                                            .size(rems(0.625))
                                            .text_color(theme.muted_foreground)
                                            .mx_0p5(),
                                    )
                                })
                        }),
                    ))
            }))
    }

    fn render_tags_section(
        &self,
        buffer: &SingleDetailBuffer,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app = self.app.clone();
        let lit_id = buffer.literature.id.clone();
        let current_tags: Vec<String> = buffer.tags.iter().map(|t| t.name.clone()).collect();
        let lit_id_selector = buffer.literature.id.clone();
        let app_selector = self.app.clone();
        let lang = self.app.current_language();
        let is_expanded = self.tags_expanded;

        let mut tags = buffer.tags.clone();
        tags.sort_by_key(|a| a.name.to_lowercase());

        v_flex()
            .group("row_group")
            .gap_2()
            .mt_2()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .id("tags-toggle")
                            .gap_1()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tags_expanded = !this.tags_expanded;
                                cx.notify();
                            }))
                            .child(
                                Icon::new(if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size(rems(0.75))
                                .text_color(theme.muted_foreground),
                            )
                            .child(
                                Label::new(t(I18nKey::Tags, lang))
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground),
                            ),
                    )
                    .child(render_icon_button(
                        "add-tag-btn",
                        IconName::Plus,
                        theme.muted_foreground,
                        theme,
                        {
                            cx.listener(move |this, event: &ClickEvent, window, cx| {
                                if let Some(parent) = &this.parent_view {
                                    let app_sel = app_selector.clone();
                                    let lit_id_sel = lit_id_selector.clone();
                                    let tags = current_tags.clone();
                                    let _ = parent.update(cx, move |parent, cx| {
                                        parent.open_tag_selector(
                                            tags,
                                            move |tag_name, _window, _cx| {
                                                let _ = app_sel.tag_service.add_tag_to_literature(
                                                    &app_sel,
                                                    &lit_id_sel,
                                                    &tag_name,
                                                );
                                            },
                                            event.position(),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                                cx.notify();
                            })
                        },
                    )),
            )
            .when(is_expanded, |this| {
                this.child(
                    h_flex()
                        .flex_wrap()
                        .gap_x_4()
                        .gap_y_2()
                        .items_center()
                        .children(tags.iter().map(|tag| {
                            let tag_name = tag.name.clone();
                            let lit_id = lit_id.clone();
                            let app = app.clone();
                            let color = tag.color.clone();
                            let tag_color = gpui::Hsla::parse_hex(&color)
                                .unwrap_or(gpui::hsla(0.6, 0.5, 0.5, 1.0));

                            h_flex()
                                .group("tag-item")
                                .gap_1p5()
                                .items_center()
                                .child(div().size(rems(0.5)).rounded_full().bg(tag_color))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.foreground)
                                        .child(tag_name.clone()),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("remove-tag-{tag_name}")))
                                        .cursor_pointer()
                                        .opacity(0.0)
                                        .group_hover("tag-item", |s| s.opacity(1.0))
                                        .child(
                                            Icon::new(IconName::Close)
                                                .size(rems(0.5))
                                                .text_color(theme.muted_foreground),
                                        )
                                        .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                            let _ = app.tag_service.remove_tag_from_literature(
                                                &app, &lit_id, &tag_name,
                                            );
                                        }),
                                )
                        })),
                )
            })
    }

    fn render_folders_section(
        &self,
        buffer: &SingleDetailBuffer,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lang = self.app.current_language();
        let is_expanded = self.folders_expanded;

        v_flex()
            .group("folders_group")
            .gap_2()
            .mt_2()
            .child(
                h_flex().justify_between().items_center().child(
                    h_flex()
                        .id("folders-toggle")
                        .gap_1()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.folders_expanded = !this.folders_expanded;
                            cx.notify();
                        }))
                        .child(
                            Icon::new(if is_expanded {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .size(rems(0.75))
                            .text_color(theme.muted_foreground),
                        )
                        .child(
                            Label::new(t(I18nKey::Folders, lang))
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.foreground),
                        ),
                ),
            )
            .when(is_expanded, |this| {
                this.child(self.render_folder_paths(buffer, theme, lang))
            })
    }

    fn render_citation_row_static(
        &self,
        target_lit: &Literature,
        current_lit_id: &str,
        is_reference: bool,
        theme: &Theme,
    ) -> impl IntoElement {
        let app = self.app.clone();
        let target_id = target_lit.id.clone();
        let source_id = if is_reference {
            current_lit_id.to_string()
        } else {
            target_lit.id.clone()
        };
        let target_id_for_removal = if is_reference {
            target_lit.id.clone()
        } else {
            current_lit_id.to_string()
        };
        let app_for_remove = app.clone();

        let this_view = self.parent_view.clone();

        div()
            .group("citation-row")
            .flex()
            .justify_between()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded_sm()
            .hover(|s| s.bg(theme.accent.opacity(0.1)))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, {
                        let target_id = target_id.clone();
                        let this_view = this_view.clone();
                        move |_, _, cx| {
                            if let Some(parent) =
                                this_view.as_ref().and_then(gpui::WeakEntity::upgrade)
                            {
                                parent.update(cx, |mw, cx| {
                                    mw.select_literature(target_id.clone(), cx);
                                });
                            }
                        }
                    })
                    .child(
                        Icon::new(IconName::FileSolid)
                            .size(rems(0.625))
                            .text_color(theme.muted_foreground)
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(target_lit.title.clone()),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .cursor_pointer()
                    .child(
                        Icon::new(IconName::Close)
                            .size(rems(0.625))
                            .text_color(theme.muted_foreground),
                    )
                    .hover(|s| s.text_color(theme.danger))
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        let _ = app_for_remove
                            .db
                            .remove_citation(&source_id, &target_id_for_removal);
                        app_for_remove.notify_data_changed();
                    }),
            )
    }

    fn render_citations_section(
        &self,
        buffer: &SingleDetailBuffer,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app = self.app.clone();
        let lit_id = buffer.literature.id.clone();
        let references = buffer.references.clone();
        let cited_by = buffer.cited_by.clone();
        let parent_view = self.parent_view.clone();
        let theme_clone = theme.clone();
        let lang = self.app.current_language();
        let is_expanded = self.citations_expanded;

        v_flex()
            .group("row_group")
            .gap_2()
            .mt_2()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .id("citations-toggle")
                            .gap_1()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.citations_expanded = !this.citations_expanded;
                                cx.notify();
                            }))
                            .child(
                                Icon::new(if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size(rems(0.75))
                                .text_color(theme.muted_foreground),
                            )
                            .child(
                                Label::new(t(I18nKey::RelatedLiterature, lang))
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground),
                            ),
                    )
                    .child(render_icon_button(
                        "add-citation-btn",
                        IconName::Plus,
                        theme.muted_foreground,
                        theme,
                        cx.listener(move |_this, _, _window, cx| {
                            if let Some(parent) = &parent_view {
                                let app = app.clone();
                                let lit_id = lit_id.clone();
                                let _ = parent.update(cx, move |parent, cx| {
                                    parent.open_citation_selector(
                                        lit_id.clone(),
                                        move |target_id, _window, _cx| {
                                            let _ = app.db.add_citation(&lit_id, &target_id);
                                            app.notify_data_changed();
                                        },
                                        cx,
                                    );
                                });
                            }
                        }),
                    )),
            )
            .when(is_expanded, |this| {
                this.child(
                    v_flex()
                        .gap_2()
                        .when(!references.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(theme_clone.muted_foreground)
                                    .child(t(I18nKey::References, lang)),
                            )
                            .children(references.iter().map(|lit| {
                                self.render_citation_row_static(
                                    lit,
                                    &buffer.literature.id,
                                    true,
                                    &theme_clone,
                                )
                            }))
                        })
                        .when(!cited_by.is_empty(), |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(theme_clone.muted_foreground)
                                    .mt_2()
                                    .child(t(I18nKey::CitedBy, lang)),
                            )
                            .children(cited_by.iter().map(|lit| {
                                self.render_citation_row_static(
                                    lit,
                                    &buffer.literature.id,
                                    false,
                                    &theme_clone,
                                )
                            }))
                        }),
                )
            })
    }

    fn render_notes_section(
        &self,
        buffer: &SingleDetailBuffer,
        window: &mut Window,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let lang = self.app.current_language();
        let is_expanded = self.notes_expanded;
        let lit_id = buffer.literature.id.clone();

        let note_cards: Vec<gpui::AnyElement> = {
            let cache = self.notes_cache.clone();
            cache
                .iter()
                .enumerate()
                .map(|(i, note)| {
                    let note_id = note.id.clone();
                    let note_title = note.title.clone();
                    let note_content = note.content.clone();

                    let this_weak = cx.entity().downgrade();
                    let et = note_title.clone();
                    let ec = note_content.clone();
                    let note_id_edit = note_id.clone();
                    let on_edit =
                        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut gpui::App| {
                            let _ = this_weak.update(cx, |this, cx| {
                                if let Some(current_idx) =
                                    this.notes_cache.iter().position(|n| n.id == note_id_edit)
                                {
                                    this.editing_note_index = Some(current_idx);
                                    let entity = cx
                                        .new(|cx| InputState::new(window, cx).placeholder("标题"));
                                    entity.update(cx, |s, cx| {
                                        s.set_value(&et, window, cx);
                                    });
                                    this.edit_note_title = Some(entity);
                                    let entity2 =
                                        cx.new(|cx| InputState::new(window, cx).multi_line(true));
                                    entity2.update(cx, |s, cx| {
                                        s.set_value(&ec, window, cx);
                                    });
                                    this.edit_note_content = Some(entity2);
                                    cx.notify();
                                }
                            });
                        };

                    let this_weak = cx.entity().downgrade();
                    let note_id_del = note_id.clone();
                    let on_delete =
                        move |_: &gpui::ClickEvent, _window: &mut Window, cx: &mut gpui::App| {
                            let _ = this_weak.update(cx, |this, cx| {
                                let _ = this.app.db.delete_note(&note_id_del);
                                this.notes_cache.retain(|n| n.id != note_id_del);
                                this.app.notify_data_changed();
                                cx.notify();
                            });
                        };

                    let this_weak = cx.entity().downgrade();
                    let note_id_exp = note_id.clone();
                    let on_toggle_expand =
                        move |_: &gpui::ClickEvent, _window: &mut Window, cx: &mut gpui::App| {
                            let _ = this_weak.update(cx, |this, cx| {
                                if this.expanded_notes.contains(&note_id_exp) {
                                    this.expanded_notes.remove(&note_id_exp);
                                } else {
                                    this.expanded_notes.insert(note_id_exp.clone());
                                }
                                cx.notify();
                            });
                        };

                    let is_note_expanded = self.expanded_notes.contains(&note_id);

                    pdf::render_shared_note_card(
                        i,
                        note,
                        is_note_expanded,
                        theme.clone(),
                        window,
                        cx,
                        on_edit,
                        on_delete,
                        on_toggle_expand,
                    )
                    .into_any_element()
                })
                .collect()
        };

        v_flex()
            .group("row_group")
            .gap_2()
            .mt_2()
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .id("notes-toggle")
                            .gap_1()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.notes_expanded = !this.notes_expanded;
                                cx.notify();
                            }))
                            .child(
                                Icon::new(if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size(rems(0.75))
                                .text_color(theme.muted_foreground),
                            )
                            .child(
                                Label::new(t(I18nKey::Notes, lang))
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(render_icon_button(
                                "ai-summary-btn",
                                IconName::Star,
                                if self.is_generating_summary {
                                    theme.primary
                                } else {
                                    theme.muted_foreground
                                },
                                theme,
                                cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    if this.is_generating_summary {
                                        return;
                                    }
                                    this.generate_ai_summary(window, cx);
                                }),
                            ))
                            .child(render_icon_button(
                                "add-note-btn",
                                IconName::Plus,
                                theme.muted_foreground,
                                theme,
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let title = "".to_string();
                                    let now = chrono::Utc::now().timestamp();
                                    this.notes_cache.push(models::LiteratureNote {
                                        id: "temp_new_note".to_string(),
                                        literature_id: lit_id.clone(),
                                        title,
                                        content: String::new(),
                                        sort_order: this.notes_cache.len() as i32,
                                        created_at: now,
                                        updated_at: now,
                                    });
                                    this.editing_note_index = Some(this.notes_cache.len() - 1);
                                    this.edit_note_title = None;
                                    this.edit_note_content = None;
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .when(is_expanded, |this| {
                if note_cards.is_empty() {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .py_2()
                            .child(t(I18nKey::NoNotes, lang)),
                    )
                } else {
                    this.children(note_cards)
                }
            })
    }

    fn render_reading_status_switcher(
        &self,
        current_status: ReadingStatus,
        lit_id: &str,
        theme: &Theme,
        lang: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app = self.app.clone();
        let lit_id = lit_id.to_string();

        let unread_label = match lang {
            Language::ZhCn => "未读",
            _ => "Unread",
        };
        let to_read_label = match lang {
            Language::ZhCn => "将读",
            _ => "To Read",
        };
        let reading_label = match lang {
            Language::ZhCn => "正读",
            _ => "Reading",
        };
        let read_label = match lang {
            Language::ZhCn => "已读",
            _ => "Read",
        };

        h_flex().gap_2().children(
            [
                (
                    ReadingStatus::Unread,
                    "Unread",
                    theme.muted_foreground,
                    unread_label,
                ),
                (ReadingStatus::ToRead, "ToRead", theme.green, to_read_label),
                (
                    ReadingStatus::Reading,
                    "Reading",
                    theme.yellow,
                    reading_label,
                ),
                (ReadingStatus::Read, "Read", gpui::rgb(0xA0522D).into(), read_label),
            ]
            .into_iter()
            .enumerate()
            .map(|(idx, (status, _key, color, label))| {
                let is_active = current_status == status;
                let status_clone = status;
                let lit_id_clone = lit_id.clone();
                let app_clone = app.clone();

                div()
                    .id(("reading-status", idx))
                    .flex()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .on_click(cx.listener(move |_this, _event, _window, cx| {
                        info!(
                            "详情: 阅读状态切换 id={}, status={:?}",
                            lit_id_clone, status_clone
                        );
                        let _ = app_clone
                            .literature_service
                            .update_literature_reading_status(
                                &app_clone,
                                &lit_id_clone,
                                status_clone,
                            );
                        app_clone.notify_data_changed();
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(rems(0.75))
                            .h(rems(0.75))
                            .rounded_full()
                            .border_1()
                            .border_color(if is_active {
                                color
                            } else {
                                theme.muted_foreground
                            })
                            .bg(if is_active {
                                color
                            } else {
                                gpui::transparent_black()
                            }),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if is_active {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .child(label),
                    )
            }),
        )
    }

    fn render_title_section(
        &self,
        title: &str,
        reading_status: ReadingStatus,
        lit_id: &str,
        theme: &Theme,
        lang: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("lit-title-wrapper")
            .on_click({
                let title = title.to_string();
                move |event, _, cx| {
                    if event.click_count() == 2 {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(title.clone()));
                    }
                }
            })
            .child(
                v_flex()
                    .group("row_group")
                    .items_start()
                    .gap_1()
                    .child(
                        Label::new(title.to_string())
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .line_clamp(10),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .items_center()
                            .gap_4()
                            .child(self.render_reading_status_switcher(
                                reading_status,
                                lit_id,
                                theme,
                                lang,
                                cx,
                            ))
                            .child(crate::ui::components::detail_helper::render_copy_button(
                                "copy-title",
                                self.copied_field.as_ref() == Some(&"title".to_string()),
                                theme,
                                cx.listener({
                                    let title = title.to_string();
                                    move |this, _, window, cx| {
                                        this.copy_text(
                                            title.clone(),
                                            "title".to_string(),
                                            window.window_handle(),
                                            cx,
                                        );
                                    }
                                }),
                            )),
                    ),
            )
    }

    fn render_rating(
        &self,
        current_rating: i32,
        lit_id: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_hovering = self.hovered_rating > 0;
        let display_rating = if is_hovering {
            self.hovered_rating
        } else {
            current_rating
        };

        h_flex()
            .id("rating-container")
            .gap_1()
            .py_1()
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .children((1..=5).map(|i| {
                let is_filled = i <= display_rating;
                let is_preview = is_hovering && i <= self.hovered_rating;
                let app = self.app.clone();
                let lit_id = lit_id.clone();

                div()
                    .id(("rating-star", i as usize))
                    .cursor_pointer()
                    .on_mouse_move(cx.listener(move |this, _, _window, cx| {
                        cx.stop_propagation();
                        if this.hovered_rating != i {
                            this.hovered_rating = i;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |_, _, _, _cx| {
                            let target_rating = if current_rating == i { 0 } else { i };
                            info!("详情: 评分设置 id={}, rating={}/5", lit_id, target_rating);
                            if let Ok(mut lit) = app.db.get_literature(&lit_id)
                                && let Some(ref mut l) = lit
                            {
                                l.rating = target_rating;
                                let _ = app.update_literature(l.clone());
                            }
                        }),
                    )
                    .child(
                        Icon::new(if is_filled {
                            IconName::StarSolid
                        } else {
                            IconName::Star
                        })
                        .size(rems(1.0))
                        .text_color(if is_filled {
                            let base_color = theme.primary;
                            if is_preview && i > current_rating {
                                base_color.opacity(0.6)
                            } else {
                                base_color
                            }
                        } else {
                            theme.muted_foreground
                        }),
                    )
            }))
    }

    fn render_badge(&self, data: &BadgeData) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .px_1()
            .py_0p5()
            .bg(data.bg)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(data.fg)
                    .line_height(rems(0.625))
                    .child(data.text.clone()),
            )
    }

    fn render_field_row(
        &self,
        label: &str,
        value: &str,
        field_id: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        DetailRow::new(
            label.to_string(),
            value.to_string(),
            self.copied_field.as_ref() == Some(&field_id.to_string()),
            cx.listener({
                let val = value.to_string();
                let field_id = field_id.to_string();
                move |this, _, window, cx| {
                    this.copy_text(val.clone(), field_id.clone(), window.window_handle(), cx);
                }
            }),
        )
        .render(theme)
    }

    fn render_link_row(
        &self,
        label: &str,
        value: &str,
        url: &str,
        field_id: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        LinkRow::new(
            label.to_string(),
            value.to_string(),
            self.copied_field.as_ref() == Some(&field_id.to_string()),
            cx.listener({
                let val = value.to_string();
                let field_id = field_id.to_string();
                move |this, _, window, cx| {
                    this.copy_text(val.clone(), field_id.clone(), window.window_handle(), cx);
                }
            }),
            cx.listener({
                let url = url.to_string();
                move |_, _, _, _| {
                    main_window::utils::open_url(&url);
                }
            }),
        )
        .render(theme)
    }

    fn render_single_detail(
        &self,
        buffer: &SingleDetailBuffer,
        theme: &Theme,
        lang: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let literature = &buffer.literature;
        let lit_id = literature.id.clone();

        div()
            .id("literature-detail-container")
            .relative()
            .size_full()
            .bg(theme.background)
            .border_l_1()
            .border_color(theme.border)
            .on_drag_move::<ExternalPaths>(cx.listener(
                |view, event: &DragMoveEvent<ExternalPaths>, _window, cx| {
                    let is_inside = event.bounds.contains(&event.event.position);
                    if view.is_dragging != is_inside {
                        view.is_dragging = is_inside;
                        cx.notify();
                    }
                },
            ))
            .child(
                div()
                    .id("literature-detail-main")
                    .size_full()
                    .bg(theme.background)
                    .overflow_y_scroll()
                    .px_3()
                    .py_3()
                    .on_mouse_move(cx.listener(|this, _, _, cx| {
                        if this.hovered_rating != 0 {
                            this.hovered_rating = 0;
                            cx.notify();
                        }
                    }))
                    .child(
                        v_flex()
                            .child(self.render_title_section(
                                &literature.title,
                                buffer.reading_status,
                                &lit_id,
                                theme,
                                lang,
                                cx,
                            ))
                            .child(self.render_rating(buffer.rating, lit_id.clone(), theme, cx))
                            .when(!buffer.authors_text.is_empty(), |this| {
                                this.child(self.render_field_row(
                                    t(I18nKey::Authors, lang),
                                    &buffer.authors_text,
                                    "authors",
                                    theme,
                                    cx,
                                ))
                            })
                            .when(!buffer.pub_name.is_empty(), |this| {
                                this.child(
                                    DetailRow::new(
                                        t(I18nKey::Publication, lang),
                                        buffer.pub_name.clone(),
                                        self.copied_field.as_ref()
                                            == Some(&"publication".to_string()),
                                        cx.listener({
                                            let val = buffer.pub_name.clone();
                                            move |this, _, window, cx| {
                                                this.copy_text(
                                                    val.clone(),
                                                    "publication".to_string(),
                                                    window.window_handle(),
                                                    cx,
                                                );
                                            }
                                        }),
                                    )
                                    .child(
                                        h_flex()
                                            .mt_1()
                                            .gap_2()
                                            .children(
                                                buffer
                                                    .jcr_badge
                                                    .as_ref()
                                                    .map(|b| self.render_badge(b)),
                                            )
                                            .children(
                                                buffer
                                                    .cas_badge
                                                    .as_ref()
                                                    .map(|b| self.render_badge(b)),
                                            )
                                            .children(
                                                buffer
                                                    .ccf_badge
                                                    .as_ref()
                                                    .map(|b| self.render_badge(b)),
                                            ),
                                    )
                                    .render(theme),
                                )
                            })
                            .when_some(literature.year, |this, year| {
                                let date_str = match (literature.month, literature.day) {
                                    (Some(m), Some(d)) => format!("{}-{:02}-{:02}", year, m, d),
                                    (Some(m), None) => format!("{}-{:02}", year, m),
                                    _ => year.to_string(),
                                };
                                this.child(self.render_field_row(
                                    t(I18nKey::Year, lang),
                                    &date_str,
                                    "year",
                                    theme,
                                    cx,
                                ))
                            })
                            .child(
                                h_flex()
                                    .gap_4()
                                    .when_some(
                                        literature.volume.as_ref().filter(|v| !v.trim().is_empty()),
                                        |this, vol| {
                                            this.child(self.render_field_row(
                                                t(I18nKey::Volume, lang),
                                                vol,
                                                "vol",
                                                theme,
                                                cx,
                                            ))
                                        },
                                    )
                                    .when_some(
                                        literature.issue.as_ref().filter(|i| !i.trim().is_empty()),
                                        |this, iss| {
                                            this.child(self.render_field_row(
                                                t(I18nKey::Issue, lang),
                                                iss,
                                                "issue",
                                                theme,
                                                cx,
                                            ))
                                        },
                                    )
                                    .when_some(
                                        literature.pages.as_ref().filter(|p| !p.trim().is_empty()),
                                        |this, pag| {
                                            this.child(self.render_field_row(
                                                t(I18nKey::Pages, lang),
                                                pag,
                                                "pages",
                                                theme,
                                                cx,
                                            ))
                                        },
                                    ),
                            )
                            .when_some(
                                literature
                                    .publication
                                    .as_ref()
                                    .and_then(|p| p.publisher.as_ref())
                                    .filter(|p| !p.trim().is_empty()),
                                |this, pub_name| {
                                    this.child(self.render_field_row(
                                        t(I18nKey::Publisher, lang),
                                        pub_name,
                                        "publisher",
                                        theme,
                                        cx,
                                    ))
                                },
                            )
                            .when_some(literature.doi.clone(), |this, doi| {
                                if doi.trim().is_empty() {
                                    this
                                } else {
                                    let url = if doi.starts_with("http") {
                                        doi.clone()
                                    } else {
                                        format!("https://doi.org/{doi}")
                                    };
                                    this.child(self.render_link_row(
                                        t(I18nKey::Doi, lang),
                                        &doi,
                                        &url,
                                        "doi",
                                        theme,
                                        cx,
                                    ))
                                }
                            })
                            .when_some(literature.arxiv_id.clone(), |this, id| {
                                if id.trim().is_empty() {
                                    this
                                } else {
                                    let url = format!("https://arxiv.org/abs/{id}");
                                    this.child(self.render_link_row(
                                        t(I18nKey::ArXiv, lang),
                                        &id,
                                        &url,
                                        "arxiv",
                                        theme,
                                        cx,
                                    ))
                                }
                            })
                            .when_some(literature.url.clone(), |this, url| {
                                if url.trim().is_empty() {
                                    this
                                } else {
                                    this.child(self.render_link_row(
                                        t(I18nKey::Url, lang),
                                        &url,
                                        &url,
                                        "url",
                                        theme,
                                        cx,
                                    ))
                                }
                            })
                            .when(!buffer.abstract_display.is_empty(), |this| {
                                let abstract_text =
                                    literature.abstract_text.clone().unwrap_or_default();
                                this.child(
                                    CollapsibleText::new(
                                        t(I18nKey::Abstract, lang),
                                        buffer.abstract_display.clone(),
                                        self.abstract_expanded,
                                        self.copied_field.as_ref() == Some(&"abstract".to_string()),
                                        (t(I18nKey::Expand, lang), t(I18nKey::Collapse, lang)),
                                        cx.listener(|this, _, _window, cx| {
                                            this.toggle_abstract(cx);
                                        }),
                                        cx.listener({
                                            let val = abstract_text.clone();
                                            move |this, _, window, cx| {
                                                this.copy_text(
                                                    val.clone(),
                                                    "abstract".to_string(),
                                                    window.window_handle(),
                                                    cx,
                                                );
                                            }
                                        }),
                                    )
                                    .on_double_click({
                                        let val = abstract_text.clone();
                                        move |_, _, cx| {
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                val.clone(),
                                            ));
                                        }
                                    })
                                    .render(theme),
                                )
                            })
                            .child(self.render_files(literature, theme))
                            .child(self.render_tags_section(buffer, theme, cx))
                            .child(self.render_folders_section(buffer, theme, cx))
                            .child(self.render_citations_section(buffer, theme, cx))
                            .child(self.render_notes_section(buffer, window, theme, cx)),
                    ),
            )
            .when(self.is_dragging, |this| {
                let lit_id = lit_id.clone();
                this.child(self.render_drop_zone(&lit_id, lang, theme, cx))
            })
            .into_any_element()
    }

    fn render_drop_zone(
        &self,
        lit_id: &str,
        lang: Language,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app = self.app.clone();
        let lit_id_main = lit_id.to_string();
        let lit_id_att = lit_id.to_string();
        let app_main = app.clone();
        let app_att = app.clone();

        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(rems(5.0))
            .bg(theme.background.opacity(0.9))
            .border_t_1()
            .border_dashed()
            .border_color(theme.border)
            .flex()
            .gap_2()
            .p_2()
            .child(
                div()
                    .id("drop-main-file")
                    .flex_1()
                    .h_full()
                    .border_2()
                    .border_dashed()
                    .border_color(theme.border)
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(t(I18nKey::SetAsMainFile, lang)),
                    )
                    .on_drop(cx.listener({
                        let app = app_main.clone();
                        let lit_id = lit_id_main.clone();
                        let _parent = self.parent_view.clone();
                        move |this, paths: &ExternalPaths, _window, cx| {
                            this.is_dragging = false;
                            if let Some(path) = paths.paths().first()
                                && let Err(e) = app.import_file_to_literature(&lit_id, path, true)
                            {
                                error!("Failed to import main file: {e}");
                                show_notification(
                                    NotificationType::Error,
                                    format!("{}: {}", t(I18nKey::ImportFailed, lang), e),
                                    cx,
                                );
                            }
                            cx.notify();
                        }
                    })),
            )
            .child(
                div()
                    .id("drop-attachment")
                    .flex_1()
                    .h_full()
                    .border_2()
                    .border_dashed()
                    .border_color(theme.border)
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(t(I18nKey::SetAsAttachment, lang)),
                    )
                    .on_drop(cx.listener({
                        let app = app_att.clone();
                        let lit_id = lit_id_att.clone();
                        let _parent = self.parent_view.clone();
                        move |this, paths: &ExternalPaths, _window, cx| {
                            this.is_dragging = false;
                            if let Some(path) = paths.paths().first()
                                && let Err(e) = app.import_file_to_literature(&lit_id, path, false)
                            {
                                error!("Failed to import attachment: {e}");
                                show_notification(
                                    NotificationType::Error,
                                    format!("{}: {}", t(I18nKey::ImportFailed, lang), e),
                                    cx,
                                );
                            }
                            cx.notify();
                        }
                    })),
            )
    }

    fn render_files(&self, literature: &Literature, theme: &Theme) -> impl IntoElement {
        let parent_view = self.parent_view.clone();

        // 1. Calculate stable numbering mapping based on the complete list
        let file_labels = models::Attachment::compute_labels(&literature.attachments);

        let mut main_elements = Vec::new();
        let mut attachment_elements = Vec::new();

        for file in &literature.attachments {
            let path_exists = Path::new(&file.file_path).exists();
            if !path_exists {
                continue;
            }
            let display_ext = file_labels.get(&file.id).cloned().unwrap_or_else(|| {
                Path::new(&file.file_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("FILE")
                    .to_uppercase()
            });

            let att_id = file.id.clone();
            let att_id_right = file.id.clone();
            let app = self.app.clone();
            let data_store = self.data_store.clone();
            let parent = parent_view.clone();
            let file_path = file.file_path.clone();
            let file_path_pdf = file.file_path.clone();
            let parent_left = parent.clone();
            let parent_right = parent.clone();

            let badge = div()
                .text_xs()
                .bg(if file.is_main {
                    theme.primary.opacity(0.1)
                } else {
                    theme.muted
                })
                .text_color(if file.is_main {
                    theme.primary
                } else {
                    theme.muted_foreground
                })
                .px_1p5()
                .py_0p5()
                .rounded_sm()
                .when(file.is_main, |s| s.font_weight(FontWeight::BOLD))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _window, cx| {
                    cx.stop_propagation();
                    if !app.should_use_external_viewer(&file_path) {
                        if let Some(lit) = data_store
                            .read(cx)
                            .literatures
                            .iter()
                            .find(|l| l.attachments.iter().any(|a| a.id == att_id))
                            .cloned()
                            && let Some(parent) =
                                parent_left.as_ref().and_then(gpui::WeakEntity::upgrade)
                        {
                            parent.update(cx, |mw, cx| {
                                mw.open_pdf_viewer_with_path(
                                    lit,
                                    Some(PathBuf::from(&file_path_pdf)),
                                    cx,
                                );
                            });
                        }
                    } else {
                        let _ = app.open_attachment(&att_id);
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let att_id = att_id_right.clone();
                    move |event: &gpui::MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        if let Some(mw) = parent_right.as_ref().and_then(gpui::WeakEntity::upgrade)
                        {
                            mw.update(cx, |mw, cx| {
                                mw.show_context_menu(
                                    event.position,
                                    ContextMenuType::Attachment(att_id.clone()),
                                    window,
                                    cx,
                                );
                            });
                        }
                    }
                })
                .child(display_ext);

            if file.is_main {
                main_elements.push(badge.into_any_element());
            } else {
                attachment_elements.push(badge.into_any_element());
            }
        }

        let mut all_elements = Vec::new();
        all_elements.extend(main_elements);
        all_elements.extend(attachment_elements);

        if all_elements.is_empty() {
            return div().into_any_element();
        }

        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(all_elements)
            .into_any_element()
    }
}

impl Render for LiteratureDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_state(cx);
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        if let Some(index) = self.editing_note_index {
            // 确保输入框状态在新建/编辑时都被正确初始化
            if self.edit_note_title.is_none() || self.edit_note_content.is_none() {
                let note = &self.notes_cache[index];
                let title = note.title.clone();
                let content = note.content.clone();

                let entity = cx.new(|cx| InputState::new(window, cx).placeholder("输入标题..."));
                entity.update(cx, |s, cx| {
                    s.set_value(&title, window, cx);
                });
                self.edit_note_title = Some(entity);

                let entity2 = cx.new(|cx| {
                    InputState::new(window, cx)
                        .multi_line(true)
                        .placeholder("输入内容 (支持 Markdown)...")
                });
                entity2.update(cx, |s, cx| {
                    s.set_value(&content, window, cx);
                });
                self.edit_note_content = Some(entity2);
            }

            let note = &self.notes_cache[index];
            let note_id = note.id.clone();
            let muted = theme.muted_foreground;

            return div()
                .size_full()
                .bg(theme.background)
                .child(
                    v_flex()
                        .size_full()
                        .p_3()
                        .gap_3()
                        .child(
                            // ── 顶部栏：包含标题和操作按钮 ──
                            h_flex()
                                .w_full()
                                .justify_between()
                                .items_center()
                                .child(
                                    Label::new("编辑笔记")
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(muted),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "d-note-cancel-{index}"
                                            )))
                                            .ghost()
                                            .icon(IconName::Close)
                                            .compact()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if this
                                                    .notes_cache
                                                    .get(index)
                                                    .map(|n| n.id.as_str())
                                                    == Some("temp_new_note")
                                                {
                                                    this.notes_cache.remove(index);
                                                }
                                                this.editing_note_index = None;
                                                this.edit_note_title = None;
                                                this.edit_note_content = None;
                                                cx.notify();
                                            })),
                                        )
                                        .child(
                                            Button::new(SharedString::from(format!(
                                                "d-note-save-{index}"
                                            )))
                                            .ghost()
                                            .icon(IconName::Check)
                                            .compact()
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                let new_title = this
                                                    .edit_note_title
                                                    .as_ref()
                                                    .map(|e| e.read(cx).text().to_string());
                                                let new_content = this
                                                    .edit_note_content
                                                    .as_ref()
                                                    .map(|e| e.read(cx).text().to_string());

                                                let mut final_note_id = note_id.clone();
                                                let is_temp = note_id == "temp_new_note";

                                                if is_temp {
                                                    let default_title =
                                                        new_title.clone().unwrap_or_else(|| {
                                                            "未命名笔记".to_string()
                                                        });
                                                    let temp_lit_id = this.notes_cache[index]
                                                        .literature_id
                                                        .clone();
                                                    if let Ok(real_id) = this
                                                        .app
                                                        .db
                                                        .create_note(&temp_lit_id, &default_title)
                                                    {
                                                        final_note_id = real_id;
                                                    }
                                                }

                                                let _ = this.app.db.update_note(
                                                    &final_note_id,
                                                    new_title.as_deref(),
                                                    new_content.as_deref(),
                                                );
                                                if let Some(n) = this.notes_cache.get_mut(index) {
                                                    n.id = final_note_id;
                                                    if let Some(ref t) = new_title {
                                                        n.title = t.clone();
                                                    }
                                                    if let Some(ref c) = new_content {
                                                        n.content = c.clone();
                                                    }
                                                }
                                                this.editing_note_index = None;
                                                this.edit_note_title = None;
                                                this.edit_note_content = None;
                                                this.app.notify_data_changed();
                                                cx.notify();
                                            })),
                                        ),
                                ),
                        )
                         .when_some(self.edit_note_title.as_ref(), |this, e| {
                            this.child(muted_input(Input::new(e), &theme).w_full())
                        })
                        .child(
                            // ── 内容输入框，通过 div 容器包裹撑满整个侧边栏 ──
                            div()
                                .w_full()
                                .flex_grow()
                                .h_0()
                                .when_some(self.edit_note_content.as_ref(), |this, e| {
                                    this.child(muted_input(Input::new(e), &theme).w_full().h_full())
                                }),
                        ),
                )
                .into_any_element();
        }

        match &self.state.mode {
            DetailMode::None => div()
                .id("literature-detail-empty")
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .bg(theme.background)
                .child(t(I18nKey::NoLiteratureSelected, lang))
                .into_any_element(),
            DetailMode::Multiple(count) => div()
                .id("literature-detail-multiple")
                .size_full()
                .bg(theme.background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new(IconName::BookOpen)
                        .size(rems(3.0))
                        .text_color(theme.muted_foreground),
                )
                .child(div().text_lg().text_color(theme.foreground).child(tf(
                    I18nKey::SelectedCount,
                    lang,
                    &[&count.to_string()],
                )))
                .into_any_element(),
            DetailMode::Single(buffer) => {
                self.render_single_detail(buffer, &theme, lang, window, cx)
            }
        }
    }
}
