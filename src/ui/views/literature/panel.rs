use crate::RUNTIME;
use crate::services::data_store::DataStore;
use crate::services::{MainApp, SyncStatus};
use crate::ui::theme_manager::surface;
use crate::ui::views::literature::{FolderDragInfo, LiteratureDragInfo};
use crate::ui::{
    components::muted_input,
    views::main_window::{Cancel, ContextMenuType, MainWindow},
};
use components::IconName;
use gpui::prelude::*;
use std::ops::Range;

use gpui::{
    AnyElement, AppContext, Entity, Hsla, KeyDownEvent, MouseButton, MouseDownEvent, Point,
    SharedString, UniformListScrollHandle, WeakEntity, Window, div, px, rems, uniform_list,
};
use gpui_component::input::InputEvent;
use gpui_component::{
    ActiveTheme, Icon, Sizable, Theme,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
};
use i18n::{I18nKey, t};
use log::{debug, error, info, warn};
use models::{Folder, Tag};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use uuid::Uuid;

pub struct LiteraturePanel {
    app: Arc<MainApp>,
    data_store: Entity<DataStore>,
    /// 正在重命名的文件夹ID和输入框状态
    renaming: Option<(String, Entity<InputState>)>,
    /// 正在重命名的标签ID和输入框状态
    tag_renaming: Option<(String, Entity<InputState>)>,
    /// 父视图引用，用于调用 `MainWindow` 的菜单
    parent_view: WeakEntity<MainWindow>,
    /// 文件夹树虚拟滚动控制
    folder_list_scroll_handle: UniformListScrollHandle,
}

impl LiteraturePanel {
    pub fn new(
        app: Arc<MainApp>,
        data_store: Entity<DataStore>,
        parent_view: WeakEntity<MainWindow>,
    ) -> Self {
        debug!("侧栏面板: 初始化");

        Self {
            app,
            data_store,
            renaming: None,
            tag_renaming: None,
            parent_view,
            folder_list_scroll_handle: UniformListScrollHandle::new(),
        }
    }

    pub fn select_folder(&mut self, folder_id: String, cx: &mut Context<Self>) {
        debug!("侧栏: 选中文件夹 '{}'", folder_id);
        let parent = self.parent_view.clone();
        let _ = parent.update(cx, |mw, mw_cx| mw.select_folder(folder_id, mw_cx));
    }

    pub fn select_tag(&mut self, tag_id: String, cx: &mut Context<Self>) {
        debug!("侧栏: 选中标签 '{}'", tag_id);
        let parent = self.parent_view.clone();
        let _ = parent.update(cx, |mw, mw_cx| mw.select_tag(tag_id, mw_cx));
    }

    fn toggle_folder_expansion(&mut self, folder_id: String) {
        if let Ok(mut state) = self.app.local_state.write() {
            if state.expanded_folder_ids.contains(&folder_id) {
                state.expanded_folder_ids.remove(&folder_id);
            } else {
                state.expanded_folder_ids.insert(folder_id);
            }
        }
    }

    pub fn start_rename(
        &mut self,
        id: String,
        is_new: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current_name = if is_new {
            String::new()
        } else {
            let data = self.data_store.read(cx);
            data.folders
                .iter()
                .find(|f| f.id == id)
                .map(|f| f.name.clone())
                .unwrap_or_default()
        };

        let lang = self.app.current_language();
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t(I18nKey::FolderNamePlaceholder, lang))
                .default_value(current_name)
        });

        input_state.update(cx, |state: &mut InputState, cx| {
            state.focus(window, cx);
        });

        cx.subscribe(&input_state, {
            let folder_id = id.clone();
            move |this, input_state: Entity<InputState>, event, cx| {
                if let InputEvent::PressEnter { .. } | InputEvent::Blur = event {
                    let new_name = input_state.read(cx).text().to_string();
                    let new_name = new_name.trim();

                    if new_name.is_empty() {
                        if is_new {
                            let _ = this.app.delete_folder(&folder_id);
                        }
                    } else {
                        let _ = this.app.rename_folder(&folder_id, new_name.to_string());
                    }
                    this.renaming = None;
                    cx.notify();
                }
            }
        })
        .detach();

        self.renaming = Some((id, input_state));
        cx.notify();
    }

    pub fn add_folder(
        &mut self,
        parent_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(pid) = &parent_id
            && let Ok(mut state) = self.app.local_state.write()
        {
            state.expanded_folder_ids.insert(pid.clone());
        }

        let new_id = Uuid::new_v4().to_string();
        info!("侧栏: 新建文件夹 (parent={:?}, id={})", parent_id, new_id);
        let _ = self.app.add_folder(parent_id, Some(new_id.clone()));
        self.start_rename(new_id, true, window, cx);
        cx.notify();
    }

    pub fn delete_folder(&mut self, id: String, cx: &mut Context<Self>) {
        info!("侧栏: 删除文件夹 (id={})", id);
        let _ = self.app.delete_folder(&id);
        cx.notify();
    }

    pub fn start_tag_rename(
        &mut self,
        id: String,
        is_new: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("UI: 启动标签重命名交互 (ID: {id})");
        let (current_name, current_color) = {
            let data = self.data_store.read(cx);
            let fallback_color = data
                .tags
                .first()
                .map(|(t, _)| t.color.clone())
                .unwrap_or_else(|| String::new());
            data.tags
                .iter()
                .find(|(t, _)| t.id == id)
                .map(|(t, _)| (t.name.clone(), t.color.clone()))
                .unwrap_or_else(|| {
                    warn!("UI: 在内存中未找到待重命名的标签 (ID: {id})");
                    (String::new(), fallback_color)
                })
        };

        let lang = self.app.current_language();
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(t(I18nKey::TagNamePlaceholder, lang))
                .default_value(current_name)
        });

        input_state.update(cx, |state: &mut InputState, cx| {
            state.focus(window, cx);
        });

        cx.subscribe(&input_state, {
            let tag_id = id.clone();
            let color = current_color;
            let app = self.app.clone();
            move |this, input_state: Entity<InputState>, event, cx| {
                if let InputEvent::PressEnter { .. } | InputEvent::Blur = event {
                    let new_name = input_state.read(cx).text().to_string();
                    let new_name = new_name.trim();

                    if !new_name.is_empty() {
                        info!("UI: 提交标签重命名 (ID: {tag_id}, NewName: {new_name})");
                        let _ = app
                            .tag_service
                            .update_tag(app.as_ref(), &tag_id, new_name, &color);
                    } else if is_new {
                        info!("UI: 新标签名称为空，执行删除 (ID: {tag_id})");
                        let _ = app.tag_service.delete_tag(app.as_ref(), &tag_id);
                    } else {
                        info!("UI: 标签名称为空，取消重命名");
                    }
                    this.tag_renaming = None;
                    cx.notify();
                }
            }
        })
        .detach();

        self.tag_renaming = Some((id, input_state));
        cx.notify();
    }

    pub fn delete_tag(&mut self, id: String, cx: &mut Context<Self>) {
        info!("UI: 用户请求删除标签 (ID: {id})");
        let _ = self.app.tag_service.delete_tag(self.app.as_ref(), &id);
        cx.notify();
    }

    fn render_static_item(
        &self,
        props: StaticItemProps,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id_str = props.id.clone();
        let color = if props.is_selected {
            props.theme.primary_foreground
        } else {
            props.theme.foreground
        };

        div()
            .id(SharedString::from(format!("static-item-{}", props.id)))
            .py_1()
            .px_3()
            .mx_2()
            .flex()
            .items_center()
            .rounded_md()
            .when(props.is_selected, |s| {
                s.bg(props.theme.primary)
                    .text_color(props.theme.primary_foreground)
            })
            .when(!props.is_selected, |s| {
                s.hover(|s| s.bg(props.theme.primary.opacity(0.15)))
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let id = id_str.clone();
                    let parent = self.parent_view.clone();
                    move |this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        // 1. 先选中
                        this.select_folder(id.clone(), cx);
                        // 2. 如果是回收站，显示菜单
                        if id == "trash"
                            && let Some(mw) = parent.upgrade()
                        {
                            mw.update(cx, |mw, cx| {
                                mw.show_context_menu(
                                    event.position,
                                    ContextMenuType::Folder(Some("trash".to_string())),
                                    window,
                                    cx,
                                );
                            });
                        }
                    }
                }),
            )
            .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                this.select_folder(id_str.clone(), cx);
                cx.notify();
            }))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        h_flex().gap_2().child((props.icon_builder)(color)).child(
                            div()
                                .text_sm()
                                .text_color(if props.is_selected {
                                    props.theme.primary_foreground
                                } else {
                                    props.theme.foreground
                                })
                                .child(props.text),
                        ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if props.is_selected {
                                props.theme.primary_foreground
                            } else {
                                props.theme.muted_foreground
                            })
                            .child(props.count),
                    ),
            )
    }

    fn flatten_folders(
        &self,
        folders: &[Arc<Folder>],
        parent_id: Option<String>,
        depth: usize,
        expanded_ids: &HashSet<String>,
    ) -> Vec<FolderTreeEntry> {
        let mut entries = Vec::new();
        let mut children: Vec<_> = folders
            .iter()
            .filter(|f| {
                f.id != "all"
                    && f.id != "uncategorized"
                    && f.id != "trash"
                    && f.parent_id == parent_id
            })
            .collect();
        children.sort_by_key(|a| a.name.to_lowercase());

        for folder in children {
            let id = folder.id.clone();
            let has_children = folders.iter().any(|f| f.parent_id == Some(id.clone()));
            let is_expanded = expanded_ids.contains(&id);
            entries.push(FolderTreeEntry {
                folder: folder.clone(),
                depth,
                is_expanded,
                has_children,
            });
            if is_expanded {
                entries.extend(self.flatten_folders(folders, Some(id), depth + 1, expanded_ids));
            }
        }
        entries
    }
    fn render_tag_item(
        &self,
        tag: &Tag,
        selected_id: Option<&String>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = selected_id == Some(&tag.id);
        let tag_id = tag.id.clone();
        let tag_id_right = tag.id.clone();

        // 解析十六进制颜色
        let color = if tag.color.starts_with('#') && tag.color.len() >= 7 {
            let r = u8::from_str_radix(&tag.color[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&tag.color[3..5], 16).unwrap_or(0);
            let b = u8::from_str_radix(&tag.color[5..7], 16).unwrap_or(0);
            gpui::rgb(u32::from_be_bytes([0, r, g, b])).into()
        } else {
            theme.primary
        };

        let parent = self.parent_view.clone();

        div()
            .id(SharedString::from(format!("tag-item-{}", tag.id)))
            .px_1p5()
            .py_0p5()
            .rounded_sm()
            .flex()
            .items_center()
            .gap_1p5()
            .cursor_pointer()
            .when(is_selected, |s| {
                s.bg(theme.primary).text_color(theme.primary_foreground)
            })
            .when(!is_selected, |s| {
                s.hover(|s| s.bg(theme.primary.opacity(0.15)))
            })
            .on_mouse_down(MouseButton::Right, {
                let tag_id = tag_id_right.clone();
                move |event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    if let Some(mw) = parent.upgrade() {
                        mw.update(cx, |mw, cx| {
                            mw.show_context_menu(
                                event.position,
                                ContextMenuType::Tag(Some(tag_id.clone())),
                                window,
                                cx,
                            );
                        });
                    }
                }
            })
            .on_click(cx.listener({
                let tag_id = tag_id.clone();
                move |this, _, _, cx| {
                    this.select_tag(tag_id.clone(), cx);
                    cx.notify();
                }
            }))
            .child(
                div()
                    .size(rems(0.4375))
                    .rounded_full()
                    .bg(color)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(if is_selected {
                        theme.primary_foreground
                    } else {
                        theme.foreground
                    })
                    .child(tag.name.clone()),
            )
    }
}

impl Render for LiteraturePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui = cx.global::<crate::services::ui_state::UiState>();
        let (sync_status, attachment_sync_status) = if let Ok(state) = self.app.sync_state.lock() {
            (
                state.sync_status.clone(),
                state.attachment_sync_status.clone(),
            )
        } else {
            (SyncStatus::Idle, SyncStatus::Idle)
        };
        let (folders, mut tags) = {
            let ds = self.data_store.read(cx);
            (ds.folders.clone(), ds.tags.clone())
        };
        debug!(
            "[LiteraturePanel::render] 从 DataStore 读取的计数: all={}, uncategorized={}, trash={}",
            folders
                .iter()
                .find(|f| f.id == "all")
                .map_or(0, |f| f.literature_count),
            folders
                .iter()
                .find(|f| f.id == "uncategorized")
                .map_or(0, |f| f.literature_count),
            folders
                .iter()
                .find(|f| f.id == "trash")
                .map_or(0, |f| f.literature_count)
        );
        let (selected_folder_id, selected_tag_id) =
            (ui.selected_folder_id.clone(), ui.selected_tag_id.clone());
        let lang = self.app.current_language();

        // 按名称排序标签
        tags.sort_by_key(|a| a.0.name.to_lowercase());

        let parent_view = self.parent_view.clone();
        let theme = cx.theme().clone();

        div()
            .flex()
            .flex_col()
            .w_full()
            .flex_grow(1.0)
            .overflow_hidden()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .relative()
            .on_action(cx.listener(|this, _: &Cancel, _, cx| {
                if let Some((rid, _)) = this.renaming.take() {
                    let is_new = {
                        let data = this.data_store.read(cx);
                        data.folders
                            .iter()
                            .find(|f| f.id == rid)
                            .is_some_and(|f| f.name.is_empty())
                    };
                    if is_new {
                        let _ = this.app.delete_folder(&rid);
                    }
                    cx.notify();
                }
                this.tag_renaming = None;
            }))
            .child({
                let all_count = folders
                    .iter()
                    .find(|f| f.id == "all")
                    .map_or(0, |f| f.literature_count);
                let uncategorized_count = folders
                    .iter()
                    .find(|f| f.id == "uncategorized")
                    .map_or(0, |f| f.literature_count);
                let trash_count = folders
                    .iter()
                    .find(|f| f.id == "trash")
                    .map_or(0, |f| f.literature_count);

                div()
                    .flex()
                    .flex_col()
                    .flex_grow(1.0)
                    .min_h_0()
                    .child(self.render_static_item(
                        StaticItemProps {
                            icon_builder: Box::new(|color| {
                                Icon::new(IconName::BookOpen)
                                    .small()
                                    .text_color(color)
                                    .into_any_element()
                            }),
                            text: t(I18nKey::AllLiterature, lang).to_string(),
                            count: all_count.to_string(),
                            is_selected: selected_folder_id.as_ref() == Some(&"all".to_string()),
                            id: "all".to_string(),
                            theme: theme.clone(),
                        },
                        cx,
                    ))
                    .child(self.render_static_item(
                        StaticItemProps {
                            icon_builder: Box::new(|color| {
                                Icon::new(IconName::File)
                                    .size(rems(1.0))
                                    .text_color(color)
                                    .into_any_element()
                            }),
                            text: t(I18nKey::Uncategorized, lang).to_string(),
                            count: uncategorized_count.to_string(),
                            is_selected: selected_folder_id.as_ref()
                                == Some(&"uncategorized".to_string()),
                            id: "uncategorized".to_string(),
                            theme: theme.clone(),
                        },
                        cx,
                    ))
                    .child(self.render_static_item(
                        StaticItemProps {
                            icon_builder: Box::new(|color| {
                                Icon::new(IconName::Trash)
                                    .size(rems(1.0))
                                    .text_color(color)
                                    .into_any_element()
                            }),
                            text: t(I18nKey::Trash, lang).to_string(),
                            count: trash_count.to_string(),
                            is_selected: selected_folder_id.as_ref() == Some(&"trash".to_string()),
                            id: "trash".to_string(),
                            theme: theme.clone(),
                        },
                        cx,
                    ))
                    .child(div().h(rems(0.0625)).bg(theme.border).my_2().mx_4())
                    // 1. 文件夹列表 (flex_1, 占用上方剩余空间)
                    .child({
                        let parent = parent_view.clone();
                        div()
                            .id("folder-list")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            // 拖放支持：移动到根目录
                            .on_drop(cx.listener(|this, drag_info: &FolderDragInfo, _, cx| {
                                let source_folder_id = &drag_info.folder_id;

                                // 检查是否已经在根目录
                                let is_already_root = {
                                     let data = this.data_store.read(cx);
                                     data.folders.iter()
                                         .find(|f| f.id == *source_folder_id)
                                         .is_some_and(|f| f.parent_id.is_none())
                                };

                                if is_already_root {
                                    return;
                                }

                                info!("移动文件夹 {source_folder_id} -> Root");
                                 let _ = this.app.move_folder(
                                     source_folder_id,
                                     None
                                 );
                                 cx.notify();
                            }))
                            // 拖拽悬停样式 (全局区域)
                            .drag_over::<FolderDragInfo>({
                                move |style, _, _, _| {
                                    style
                                        .bg(surface().selected_faint)
                                }
                            })
                            .on_mouse_down(MouseButton::Right, move |event: &MouseDownEvent, window, cx| {
                                // 空白区域触发“新建文件夹”菜单
                                if let Some(mw) = parent.upgrade() {
                                    mw.update(cx, |mw, cx| {
                                        mw.show_context_menu(
                                            event.position,
                                            ContextMenuType::Folder(None),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                            })
                            .child({
                                // 获取展开状态
                                let expanded_ids = self
                                    .app
                                    .local_state
                                    .read()
                                    .map(|s| s.expanded_folder_ids.clone())
                                    .unwrap_or_default();
                                // 拍平成平面列表
                                let entries = Rc::new(self.flatten_folders(
                                    &folders,
                                    None,
                                    0,
                                    &expanded_ids,
                                ));
                                let entry_count = entries.len();
                                let entries_clone = entries.clone();
                                let selected_id = selected_folder_id.clone();

                                uniform_list("folder-tree", entry_count, {
                                    cx.processor(move |this, visible_range: Range<usize>, _window, cx| {
                                        let mut items = Vec::with_capacity(
                                            visible_range.len(),
                                        );
                                        let theme = cx.theme().clone();
                                        for ix in visible_range {
                                            let entry = &entries_clone[ix];
                                            let is_selected = selected_id.as_ref()
                                                == Some(&entry.folder.id);
                                            let is_renaming = this
                                                .renaming
                                                .as_ref()
                                                .is_some_and(|(rid, _)| {
                                                    rid == &entry.folder.id
                                                });

                                            let item: AnyElement = if is_renaming {
                                                let (rid, input_state) = this
                                                    .renaming
                                                    .as_ref()
                                                    .unwrap();
                                                let rid = rid.clone();
                                                let input_state = input_state.clone();
                                                div()
                                                    .px_3()
                                                    .pl(rems(
                                                        1.0 * entry.depth as f32 + 1.25,
                                                    ))
                                                    .py_0p5()
                                                    .on_key_down(cx.listener(
                                                        move |this,
                                                              event: &KeyDownEvent,
                                                              _,
                                                              cx| {
                                                            if event.keystroke.key
                                                                == "escape"
                                                            {
                                                                let is_new = {
                                                                    let data = this
                                                                        .data_store
                                                                        .read(cx);
                                                                    data.folders
                                                                        .iter()
                                                                        .find(|f| {
                                                                            f.id == rid
                                                                        })
                                                                        .is_some_and(
                                                                            |f| {
                                                                                f.name
                                                                                    .is_empty()
                                                                            },
                                                                        )
                                                                };
                                                                if is_new {
                                                                    let _ = this
                                                                        .app
                                                                        .delete_folder(
                                                                            &rid,
                                                                        );
                                                                }
                                                                this.renaming = None;
                                                                cx.notify();
                                                            }
                                                        },
                                                    ))
                                                    .child(muted_input(
                                                        Input::new(&input_state),
                                                        &theme,
                                                    ))
                                                    .into_any_element()
                                            } else {
                                                let folder_id = entry.folder.id.clone();
                                                let folder_id_right = folder_id.clone();
                                                let folder_id_drop = folder_id.clone();
                                                let folder_id_folder_drop =
                                                    folder_id.clone();
                                                let folder_id_drag = folder_id.clone();
                                                let folder_name_drag =
                                                    entry.folder.name.clone();

                                                let icon = if entry.is_expanded {
                                                    IconName::FolderOpen
                                                } else {
                                                    IconName::Folder
                                                };
                                                let chevron = if entry.has_children {
                                                    if entry.is_expanded {
                                                        Some(IconName::ChevronDown)
                                                    } else {
                                                        Some(IconName::ChevronRight)
                                                    }
                                                } else {
                                                    None
                                                };
                                                let parent_mw = this.parent_view.clone();

                                                div()
                                                    .id(SharedString::from(format!(
                                                        "folder-item-wrapper-{}",
                                                        folder_id
                                                    )))
                                                    .on_drop(cx.listener({
                                                        let folder_id =
                                                            folder_id_drop.clone();
                                                        move |this,
                                                              drag_info:
                                                              &LiteratureDragInfo,
                                                              _,
                                                              cx| {
                                                            info!(
                                                                "拖放文献到文件夹: {} 篇文献 -> {}",
                                                                drag_info.count(),
                                                                folder_id
                                                            );
                                                            for lit_id in
                                                                &drag_info
                                                                    .literature_ids
                                                            {
                                                                if let Err(e) = this
                                                                    .app
                                                                    .add_literature_to_folder(
                                                                        lit_id,
                                                                        &folder_id,
                                                                    )
                                                                {
                                                                    error!(
                                                                        "添加文献到文件夹失败: {e}"
                                                                    );
                                                                }
                                                            }
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .on_drop(cx.listener({
                                                        let target_folder_id =
                                                            folder_id_folder_drop;
                                                        move |this,
                                                              drag_info:
                                                              &FolderDragInfo,
                                                              _,
                                                              cx| {
                                                            let source_folder_id =
                                                                &drag_info.folder_id;
                                                            if source_folder_id
                                                                == &target_folder_id
                                                            {
                                                                return;
                                                            }
                                                            let data = this
                                                                .data_store
                                                                .read(cx);
                                                            let is_descendant = {
                                                                let mut current_id =
                                                                    Some(
                                                                        target_folder_id
                                                                            .clone(),
                                                                    );
                                                                let mut found = false;
                                                                while let Some(
                                                                    cid,
                                                                ) = current_id
                                                                {
                                                                    if cid
                                                                        == *source_folder_id
                                                                    {
                                                                        found = true;
                                                                        break;
                                                                    }
                                                                    if let Some(
                                                                        folder,
                                                                    ) = data
                                                                        .folders
                                                                        .iter()
                                                                        .find(|f| {
                                                                            f.id == cid
                                                                        })
                                                                    {
                                                                        current_id =
                                                                            folder
                                                                                .parent_id
                                                                                .clone();
                                                                    } else {
                                                                        break;
                                                                    }
                                                                }
                                                                found
                                                            };
                                                            if is_descendant {
                                                                warn!(
                                                                    "无法移动文件夹: 目标是源的子文件夹"
                                                                );
                                                                return;
                                                            }
                                                            let is_already_parent = data
                                                                .folders
                                                                .iter()
                                                                .find(|f| {
                                                                    f.id
                                                                        == *source_folder_id
                                                                })
                                                                .is_some_and(|f| {
                                                                    f.parent_id.as_ref()
                                                                        == Some(
                                                                            &target_folder_id,
                                                                        )
                                                                });
                                                            if is_already_parent {
                                                                return;
                                                            }
                                                            info!(
                                                                "移动文件夹 {source_folder_id} -> {target_folder_id}"
                                                            );
                                                            let _ = this
                                                                .app
                                                                .move_folder(
                                                                    source_folder_id,
                                                                    Some(
                                                                        target_folder_id
                                                                            .clone(),
                                                                    ),
                                                                );
                                                            cx.notify();
                                                        }
                                                    }))
                                                    .drag_over::<LiteratureDragInfo>({
                                                        let theme = theme.clone();
                                                        move |style, _, _, _| {
                                                            style
                                                                .bg(surface().selected_hover)
                                                                .border_1()
                                                                .border_color(
                                                                    theme.primary,
                                                                )
                                                                .rounded_md()
                                                        }
                                                    })
                                                    .drag_over::<FolderDragInfo>({
                                                        let theme = theme.clone();
                                                        move |style, _, _, _| {
                                                            style
                                                                .bg(surface().selected_hover)
                                                                .border_1()
                                                                .border_color(
                                                                    theme.primary,
                                                                )
                                                                .rounded_md()
                                                        }
                                                    })
                                                    .on_mouse_down(
                                                        MouseButton::Right,
                                                        cx.listener({
                                                            let folder_id =
                                                                folder_id_right.clone();
                                                            let parent_mw =
                                                                parent_mw.clone();
                                                            move |this,
                                                                  event:
                                                                  &MouseDownEvent,
                                                                  window,
                                                                  cx| {
                                                                cx.stop_propagation();
                                                                this.select_folder(
                                                                    folder_id.clone(),
                                                                    cx,
                                                                );
                                                                if let Some(mw) =
                                                                    parent_mw.upgrade()
                                                                {
                                                                    mw.update(
                                                                        cx,
                                                                        |mw, cx| {
                                                                            mw
                                                                                .show_context_menu(
                                                                                    event
                                                                                        .position,
                                                                                    ContextMenuType::Folder(
                                                                                        Some(
                                                                                            folder_id.clone(),
                                                                                        ),
                                                                                    ),
                                                                                    window,
                                                                                    cx,
                                                                                );
                                                                        },
                                                                    );
                                                                }
                                                            }
                                                        }),
                                                    )
                                                    .child(
                                                        div()
                                                            .id(SharedString::from(
                                                                format!(
                                                                    "folder-item-inner-{}",
                                                                    folder_id
                                                                ),
                                                            ))
                                                            .on_drag(
                                                                FolderDragInfo::new(
                                                                    folder_id_drag,
                                                                    folder_name_drag,
                                                                ),
                                                                |drag_info,
                                                                 _point,
                                                                 _window,
                                                                 cx| {
                                                                    cx.new(|_| {
                                                                        drag_info
                                                                            .clone()
                                                                            .with_position(
                                                                                Point::new(
                                                                                    px(
                                                                                        0.0,
                                                                                    ),
                                                                                    px(
                                                                                        0.0,
                                                                                    ),
                                                                                ),
                                                                            )
                                                                    })
                                                                },
                                                            )
                                                            .flex()
                                                            .items_center()
                                                            .px_3()
                                                            .py_0p5()
                                                            .pl(rems(
                                                                1.0 * entry.depth as f32
                                                                    + 0.25,
                                                            ))
                                                            .mx_2()
                                                            .rounded_md()
                                                            .when(
                                                                is_selected,
                                                                |s| {
                                                                     s.bg(
                                                                         theme.primary,
                                                                     )
                                                                    .text_color(
                                                                        theme.primary_foreground,
                                                                    )
                                                                },
                                                            )
                                                            .when(
                                                                !is_selected,
                                                                |s| {
                                                                    s.hover(|s| {
                                                                        s.bg(theme.primary.opacity(0.15))
                                                                    })
                                                                },
                                                            )
                                                            .on_click(cx.listener({
                                                                let folder_id =
                                                                    folder_id.clone();
                                                                move |this: &mut Self,
                                                                      event:
                                                                      &gpui::ClickEvent,
                                                                      _,
                                                                      cx| {
                                                                    if event
                                                                        .click_count()
                                                                        == 2
                                                                    {
                                                                        this
                                                                            .toggle_folder_expansion(
                                                                                folder_id
                                                                                    .clone(),
                                                                            );
                                                                    } else {
                                                                        this
                                                                            .select_folder(
                                                                                folder_id
                                                                                    .clone(),
                                                                                cx,
                                                                            );
                                                                    }
                                                                    cx.notify();
                                                                }
                                                            }))
                                                            .child(
                                                                h_flex()
                                                                    .w_full()
                                                                    .justify_between()
                                                                    .child(
                                                                        h_flex()
                                                                            .gap_1()
                                                                            .child(
                                                                                div()
                                                                                    .id(
                                                                                        SharedString::from(
                                                                                            format!(
                                                                                                "folder-item-chevron-{}",
                                                                                                folder_id
                                                                                            ),
                                                                                        ),
                                                                                    )
                                                                                    .w(
                                                                                        rems(
                                                                                            0.75,
                                                                                        ),
                                                                                    )
                                                                                    .flex()
                                                                                    .items_center()
                                                                                    .justify_center()
                                                                                    .cursor_pointer()
                                                                                    .on_click(
                                                                                        cx
                                                                                            .listener(
                                                                                        {
                                                                                            let folder_id =
                                                                                                folder_id
                                                                                                    .clone(
                                                                                                );
                                                                                            move |this: &mut Self,
                                                                                                  _,
                                                                                                  _,
                                                                                                  cx| {
                                                                                                cx.stop_propagation(
                                                                                                );
                                                                                                this
                                                                                                    .toggle_folder_expansion(
                                                                                                        folder_id
                                                                                                            .clone(
                                                                                                        ),
                                                                                                    );
                                                                                                cx.notify(
                                                                                                );
                                                                                            }
                                                                                        },
                                                                                        ),
                                                                                    )
                                                                                    .children(
                                                                                        chevron
                                                                                            .map(
                                                                                            |c| {
                                                                                                Icon::new(
                                                                                                    c,
                                                                                                )
                                                                                                .xsmall()
                                                                                                .text_color(
                                                                                                    if is_selected { theme.primary_foreground } else { theme.muted_foreground },
                                                                                                )
                                                                                            },
                                                                                        ),
                                                                                    ),
                                                                            )
                                                                            .child(
                                                                                Icon::new(
                                                                                    icon,
                                                                                )
                                                                                .small()
                                                                                .text_color(
                                                                                    if is_selected { theme.primary_foreground } else { theme.foreground },
                                                                                ),
                                                                            )
                                                                            .child(
                                                                                div()
                                                                                    .text_sm()
                                                                                    .text_color(
                                                                                        if is_selected { theme.primary_foreground } else { theme.foreground },
                                                                                    )
                                                                                    .child(
                                                                                        entry
                                                                                            .folder
                                                                                            .name
                                                                                            .clone(),
                                                                                    ),
                                                                            ),
                                                                    )
                                                                    .child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(
                                                                                if is_selected { theme.primary_foreground } else { theme.muted_foreground },
                                                                            )
                                                                            .child(
                                                                                entry
                                                                                    .folder
                                                                                    .literature_count
                                                                                    .to_string(),
                                                                            ),
                                                                    ),
                                                            ),
                                                    )
                                                    .into_any_element()
                                            };
                                            items.push(item);
                                        }
                                        items
                                    })
                                })
                                .flex_grow_1()
                                .size_full()
                                .track_scroll(&self.folder_list_scroll_handle)
                            })
                    })
                    // 2. 标签容器
                    .child(
                        div()
                            .id("tag-container")
                            .flex()
                            .flex_col()
                            .flex_shrink_0()
                            .max_h(rems(12.5))
                            .overflow_y_scroll()
                            .border_t_1()
                            .border_color(surface().border_faint)
                            .bg(theme.sidebar)
                            .child(
                                div()
                                    .id("tag-scroll-list")
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .gap_x_2()
                                    .gap_y_1()
                                    .p_3()
                                    .children(tags.iter().map(|(tag, _count)| {
                                            if let Some((rid, input_state)) = &self.tag_renaming
                                                && rid == &tag.id
                                            {
                                                let rid = rid.clone();
                                                return div()
                                                        .w_full()
                                                        .mb_1()
                                                        .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                                                             if event.keystroke.key == "escape" {
                                                                 let is_new = {
                                                                     let data = this.data_store.read(cx);
                                                                     data.tags
                                                                         .iter()
                                                                         .find(|(t, _)| t.id == rid)
                                                                         .is_some_and(|(t, _)| t.name.is_empty())
                                                                 };

                                                                if is_new {
                                                                    let _ = this.app.tag_service.delete_tag(this.app.as_ref(), &rid);
                                                                }
                                                                this.tag_renaming = None;
                                                                cx.notify();
                                                            }
                                                        }))
                                                        .child(muted_input(Input::new(input_state), &theme))
                                                        .into_any_element();
                                            }
                                            self.render_tag_item(tag, selected_tag_id.as_ref(), &theme, cx).into_any_element()
                                        }))
                                )
                    )
            })
            .child(
                h_flex()
                    .flex_shrink_0()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(surface().border_faint)
                    .child({
                        let icon = match &sync_status {
                            SyncStatus::Idle => Icon::new(IconName::Check)
                                .small()
                                .text_color(theme.muted_foreground),
                            SyncStatus::Syncing => Icon::new(IconName::LoaderCircle)
                                .small()
                                .text_color(theme.primary),
                            SyncStatus::Error(_) => {
                                Icon::new(IconName::CircleX).small().text_color(theme.red_light)
                            }
                            SyncStatus::Conflict(_) => Icon::new(IconName::TriangleAlert)
                                .small()
                                .text_color(theme.warning),
                        };
                        div().relative().child(
                            Button::new("btn-sync-status")
                                .child(icon)
                                .ghost()
                                .xsmall()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    match &sync_status {
                                        SyncStatus::Idle | SyncStatus::Error(_) => {
                                            // 点击时直接触发重试，不显示旧的错误信息
                                            let app = this.app.clone();
                                            RUNTIME.spawn(async move {
                                                app.sync_service.force_sync(app.clone()).await;
                                            });
                                        }
                                        SyncStatus::Conflict(lits) => {
                                            // 获取本地对应的文献，构成对比组
                                            let mut groups = Vec::new();
                                            {
                                                    for remote_lit in lits {
                                                    if let Ok(Some(local_lit)) =
                                                        this.app.db.get_literature(&remote_lit.id)
                                                    {
                                                        groups.push(vec![
                                                            local_lit,
                                                            remote_lit.clone(),
                                                        ]);
                                                    } else {
                                                        groups.push(vec![remote_lit.clone()]);
                                                    }
                                                }
                                            }
                                            if let Ok(mut state) = this.app.sync_state.lock() {
                                                state.sync_conflict_groups = Some(groups);
                                            }
                                            // 发射 Action 让 MainWindow 捕获并打开冲突列表
                                            cx.dispatch_action(
                                                &crate::ui::views::main_window::HandleSyncConflicts,
                                            );
                                        }
                                        SyncStatus::Syncing => {}
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("btn-sync-attachments")
                            .child(match &attachment_sync_status {
                                SyncStatus::Syncing => Icon::new(IconName::LoaderCircle)
                                    .small()
                                    .text_color(theme.primary),
                                SyncStatus::Error(_) => Icon::new(IconName::TriangleAlert)
                                    .small()
                                    .text_color(theme.red_light),
                                _ => Icon::new(IconName::Cloud).small().text_color(theme.muted_foreground),
                            })
                            .ghost()
                            .xsmall()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let SyncStatus::Syncing = attachment_sync_status {
                                    return;
                                }

                                // 点击直接重试，不显示旧错误
                                let app = this.app.clone();
                                RUNTIME.spawn(async move {
                                    app.perform_attachments_sync();
                                });
                                    cx.notify();
                            })),
                    ),
            )
    }
}

struct FolderTreeEntry {
    folder: Arc<Folder>,
    depth: usize,
    is_expanded: bool,
    has_children: bool,
}
struct StaticItemProps {
    icon_builder: Box<dyn Fn(Hsla) -> AnyElement>,
    text: String,
    count: String,
    is_selected: bool,
    id: String,
    theme: Theme,
}
