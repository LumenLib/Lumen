use crate::notification_bus::show_notification;
use crate::services::MainApp;
use crate::services::data_store::DataStore;
use crate::ui::{
    components::{CollapsibleText, DetailRow, LinkRow, render_icon_button},
    icons::IconName,
    views::main_window::{self, ContextMenuType, MainWindow},
};
use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, AsyncApp, ClickEvent, DragMoveEvent, Entity, ExternalPaths, FontWeight,
    MouseButton, SharedString, WeakEntity, Window, div, px, rems,
};
use gpui_component::{
    ActiveTheme, Colorize, Icon, Theme,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::NotificationType,
    text::TextView,
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
    literature: Literature,
    ccf_badge: Option<BadgeData>,
    jcr_badge: Option<BadgeData>,
    cas_badge: Option<BadgeData>,
    authors_text: String,
    pub_name: String,
    abstract_display: String,
    rating: i32,
    tags: Vec<TagData>,
    references: Vec<Literature>,
    cited_by: Vec<Literature>,
    reading_status: ReadingStatus,
    folder_paths: Vec<Vec<String>>,
    notes_text: String,
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
    /// 笔记是否处于编辑模式
    notes_edit_mode: bool,
    /// 笔记编辑输入框状态
    notes_input_state: Option<Entity<InputState>>,
    /// 父视图句柄 (`MainWindow`)
    parent_view: Option<WeakEntity<MainWindow>>,
    /// 预实体化缓冲状态
    state: DetailState,
    /// 鼠标当前悬停的评分值（用于预览）
    hovered_rating: i32,
    /// Copy feedback state
    copied_field: Option<String>,
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
            notes_edit_mode: false,
            notes_input_state: None,
            parent_view: None,
            state: DetailState {
                selected_ids: Vec::new(),
                content_version: -1,
                mode: DetailMode::None,
            },
            hovered_rating: 0,
            copied_field: None,
        }
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
        let tags = Self::build_tags(&lit, &store);
        let references = self.build_references(&lit, &store);
        let cited_by = self.build_cited_by(&lit, &store);
        let folder_paths = Self::build_folder_paths(&lit, &store, self.app.current_language());

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
            notes_text: lit.notes.clone().unwrap_or_default(),
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

    fn build_references(&self, lit: &Literature, store: &DataStore) -> Vec<Literature> {
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

    fn build_cited_by(&self, lit: &Literature, store: &DataStore) -> Vec<Literature> {
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
        tags.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

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
        let is_editing = self.notes_edit_mode;
        let notes_text = buffer.notes_text.clone();
        let notes_text_for_handler = notes_text.clone();

        v_flex()
            .group("row_group")
            .gap_2()
            .mt_2()
            .child(
                h_flex().justify_between().items_center().child(
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
                ),
            )
            .when(is_expanded, |this| {
                if is_editing {
                    if let Some(input) = &self.notes_input_state {
                        this.child(
                            v_flex()
                                .gap_2()
                                .pt_2()
                                .child(
                                    Label::new(t(I18nKey::EditNotesMarkdown, lang))
                                        .text_xs()
                                        .text_color(theme.muted_foreground),
                                )
                                .child(Input::new(input).w_full().h(px(200.0)))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .justify_end()
                                        .child(
                                            Button::new("notes-cancel")
                                                .ghost()
                                                .label(t(I18nKey::Cancel, lang))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.notes_edit_mode = false;
                                                        this.notes_input_state = None;
                                                        this.abstract_expanded =
                                                            !this.abstract_expanded;
                                                        cx.notify();
                                                    }),
                                                ),
                                        )
                                        .child(
                                            Button::new("notes-save")
                                                .label(t(I18nKey::Save, lang))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        if let Some(input) = &this.notes_input_state
                                                        {
                                                            let text =
                                                                input.read(cx).text().to_string();
                                                            if let DetailMode::Single(ref buffer) =
                                                                this.state.mode
                                                            {
                                                                let id =
                                                                    buffer.literature.id.clone();
                                                                info!(
                                                                    "详情: 笔记已保存 (id={})",
                                                                    id
                                                                );
                                                                let _ = this
                                                                    .app
                                                                    .db
                                                                    .update_literature_notes(
                                                                        &id, &text,
                                                                    );
                                                                this.app.notify_data_changed();
                                                            }
                                                        }
                                                        this.notes_edit_mode = false;
                                                        this.notes_input_state = None;
                                                        this.sync_state(cx);
                                                    }),
                                                ),
                                        ),
                                ),
                        )
                    } else {
                        this.child(div())
                    }
                } else if notes_text.is_empty() {
                    this.child(
                        v_flex().pt_2().gap_2().child(
                            h_flex().justify_end().child(
                                Button::new("notes-add-btn")
                                    .ghost()
                                    .label(t(I18nKey::Add, lang))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, window, cx| {
                                            this.notes_edit_mode = true;
                                            let entity = cx.new(|cx| {
                                                InputState::new(window, cx).multi_line(true)
                                            });
                                            entity.update(cx, |state, cx| {
                                                state.set_value(
                                                    &notes_text_for_handler,
                                                    window,
                                                    cx,
                                                );
                                            });
                                            this.notes_input_state = Some(entity);
                                            cx.notify();
                                        }),
                                    ),
                            ),
                        ),
                    )
                } else {
                    this.child(
                        div().pt_2().child(
                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex().justify_end().child(
                                        Button::new("notes-edit-btn")
                                            .ghost()
                                            .label(t(I18nKey::Edit, lang))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, window, cx| {
                                                    this.notes_edit_mode = true;
                                                    if this.notes_input_state.is_none() {
                                                        let entity = cx.new(|cx| {
                                                            InputState::new(window, cx)
                                                                .multi_line(true)
                                                        });
                                                        entity.update(cx, |state, cx| {
                                                            state.set_value(
                                                                &notes_text_for_handler,
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                        this.notes_input_state = Some(entity);
                                                    }
                                                    cx.notify();
                                                }),
                                            ),
                                    ),
                                )
                                .child(
                                    TextView::markdown("detail-notes", &notes_text, window, cx)
                                        .selectable(true),
                                ),
                        ),
                    )
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

        h_flex().gap_2().children(
            [
                (
                    ReadingStatus::Unread,
                    "Unread",
                    theme.blue,
                    t(I18nKey::Unread, lang),
                ),
                (
                    ReadingStatus::Reading,
                    "Reading",
                    theme.green,
                    t(I18nKey::StatusReading, lang),
                ),
                (
                    ReadingStatus::Read,
                    "Read",
                    theme.yellow,
                    t(I18nKey::StatusRead, lang),
                ),
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
                            if let Ok(mut lit) = app.db.get_literature(&lit_id) {
                                if let Some(ref mut l) = lit {
                                    l.rating = target_rating;
                                    let _ = app.update_literature(l.clone());
                                }
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
                                    &t(I18nKey::Authors, lang),
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
                                this.child(self.render_field_row(
                                    &t(I18nKey::Year, lang),
                                    &year.to_string(),
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
                                                &t(I18nKey::Volume, lang),
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
                                                &t(I18nKey::Issue, lang),
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
                                                &t(I18nKey::Pages, lang),
                                                pag,
                                                "pages",
                                                theme,
                                                cx,
                                            ))
                                        },
                                    ),
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
                                        &t(I18nKey::Doi, lang),
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
                                        &t(I18nKey::ArXiv, lang),
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
                                        &t(I18nKey::Url, lang),
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
                                show_notification(NotificationType::Error, format!("{}: {}", t(I18nKey::ImportFailed, lang), e.to_string()), cx);
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
                                show_notification(NotificationType::Error, format!("{}: {}", t(I18nKey::ImportFailed, lang), e.to_string()), cx);
                            }
                            cx.notify();
                        }
                    })),
            )
    }

    fn render_files(&self, literature: &Literature, theme: &Theme) -> impl IntoElement {
        let mut main_elements = Vec::new();
        let mut attachment_elements = Vec::new();
        let parent_view = self.parent_view.clone();

        for file in &literature.attachments {
            let path_exists = Path::new(&file.file_path).exists();
            if !path_exists {
                continue;
            }
            let ext = Path::new(&file.file_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("FILE")
                .to_uppercase();

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
                        {
                            if let Some(parent) =
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
                        }
                    } else {
                        let _ = app.open_attachment(&att_id);
                    }
                })
                .on_mouse_down(MouseButton::Right, {
                    let att_id = att_id_right.clone();
                    move |event: &gpui::MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if let Some(mw) = parent_right.as_ref().and_then(gpui::WeakEntity::upgrade)
                        {
                            mw.update(cx, |mw, cx| {
                                mw.show_context_menu(
                                    event.position,
                                    ContextMenuType::Attachment(att_id.clone()),
                                    cx,
                                );
                            });
                        }
                    }
                })
                .child(ext);

            if file.is_main {
                main_elements.push(badge.into_any_element());
            } else {
                attachment_elements.push(badge.into_any_element());
            }
        }

        if main_elements.is_empty() && attachment_elements.is_empty() {
            return div().into_any_element();
        }

        let lang = self.app.current_language();

        v_flex()
            .gap_3()
            .when(!main_elements.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t(I18nKey::MainFile, lang)),
                        )
                        .child(div().flex().flex_wrap().gap_2().children(main_elements)),
                )
            })
            .when(!attachment_elements.is_empty(), |this| {
                this.child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(t(I18nKey::Attachment, lang)),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_2()
                                .children(attachment_elements),
                        ),
                )
            })
            .into_any_element()
    }
}

impl Render for LiteratureDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_state(cx);
        let theme = cx.theme().clone();
        let lang = self.app.current_language();

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
