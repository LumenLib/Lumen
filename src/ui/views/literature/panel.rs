use crate::RUNTIME;
use crate::services::data_store::DataStore;
use crate::services::{AppViewMode, MainApp, SyncStatus};
use crate::ui::views::literature::{FolderDragInfo, LiteratureDragInfo};
use crate::ui::{
    icons::IconName,
    views::main_window::{Cancel, ContextMenuType, MainWindow},
};
use gpui::prelude::*;
use gpui::{
    AnyElement, AppContext, Entity, FontWeight, Hsla, KeyDownEvent, MouseButton, MouseDownEvent,
    Point, SharedString, WeakEntity, Window, WindowControlArea, div, px, rems,
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
use std::sync::Arc;
use uuid::Uuid;

pub struct LiteraturePanel {
    app: Arc<MainApp>,
    data_store: Entity<DataStore>,
    /// 正在重命名的文件夹ID和输入框状态
    renaming: Option<(String, Entity<InputState>)>,
    /// 正在重命名的标签ID和输入框状态
    tag_renaming: Option<(String, Entity<InputState>)>,
    /// 标签栏是否展开 (TODO: 也可移入 `local_state`)
    tags_expanded: bool,
    /// 父视图引用，用于调用 `MainWindow` 的菜单
    parent_view: WeakEntity<MainWindow>,
}

impl LiteraturePanel {
    pub fn new(
        app: Arc<MainApp>,
        data_store: Entity<DataStore>,
        parent_view: WeakEntity<MainWindow>,
    ) -> Self {
        debug!("侧栏面板: 初始化");
        let tags_expanded = if let Ok(state) = app.local_state.read() {
            state.tags_sidebar_expanded
        } else {
            true
        };

        Self {
            app,
            data_store,
            renaming: None,
            tag_renaming: None,
            tags_expanded,
            parent_view,
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
            data.tags
                .iter()
                .find(|(t, _)| t.id == id)
                .map(|(t, _)| (t.name.clone(), t.color.clone()))
                .unwrap_or_else(|| {
                    warn!("UI: 在内存中未找到待重命名的标签 (ID: {id})");
                    (String::new(), "#808080".to_string())
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

    pub fn add_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("UI: 用户请求创建新标签");
        // 创建一个空名称的标签，以便用户输入
        match self.app.tag_service.create_tag(self.app.as_ref(), "", None) {
            Ok(tag) => {
                let id = tag.id.clone();
                info!("UI: 新标签已持久化 (ID: {id}), 启动即时重命名");
                self.start_tag_rename(id, true, window, cx);
            }
            Err(e) => error!("UI: 创建标签失败: {e}"),
        }
    }

    pub fn delete_tag(&mut self, id: String, cx: &mut Context<Self>) {
        info!("UI: 用户请求删除标签 (ID: {id})");
        let _ = self.app.tag_service.delete_tag(self.app.as_ref(), &id);
        cx.notify();
    }

    fn toggle_tags_expansion(&mut self, cx: &mut Context<Self>) {
        self.tags_expanded = !self.tags_expanded;
        if let Ok(mut state) = self.app.local_state.write() {
            state.tags_sidebar_expanded = self.tags_expanded;
        }
        cx.notify();
    }

    fn render_static_item(
        &self,
        props: StaticItemProps,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let id_str = props.id.clone();
        let color = if props.is_selected {
            props.theme.primary
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
                s.bg(props.theme.primary.opacity(0.1))
                    .text_color(props.theme.primary)
            })
            .when(!props.is_selected, |s| s.hover(|s| s.bg(props.theme.muted)))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let id = id_str.clone();
                    let parent = self.parent_view.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
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
                                    props.theme.primary
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
                                props.theme.primary.opacity(0.8)
                            } else {
                                props.theme.muted_foreground
                            })
                            .child(props.count),
                    ),
            )
    }

    fn render_folder_tree(
        &self,
        props: FolderTreeProps,
        cx: &mut Context<Self>,
    ) -> Vec<impl IntoElement> {
        let mut elements = Vec::new();
        let mut target_folders: Vec<_> = props
            .folders
            .iter()
            .filter(|f| {
                f.id != "all"
                    && f.id != "uncategorized"
                    && f.id != "trash"
                    && f.parent_id == props.parent_id
            })
            .collect();

        target_folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        for folder in target_folders {
            let id = folder.id.clone();
            let is_renaming = props.renaming.is_some_and(|(rid, _)| rid == &id);
            let is_expanded = if let Ok(state) = self.app.local_state.read() {
                state.expanded_folder_ids.contains(&id)
            } else {
                false
            };
            let has_children = props
                .folders
                .iter()
                .any(|f| f.parent_id == Some(id.clone()));

            if is_renaming {
                let (rid, input_state) = props.renaming.unwrap();
                let rid = rid.clone();
                let input_state = input_state.clone();
                elements.push(
                    div()
                        .px_3()
                        .pl(rems(1.0 * props.depth as f32 + 1.25))
                        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "escape" {
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
                                this.renaming = None;
                                cx.notify();
                            }
                        }))
                        .child(Input::new(&input_state))
                        .into_any_element(),
                );
            } else {
                elements.push(
                    self.render_folder_item(
                        FolderItemProps {
                            folder: folder.clone(),
                            selected_id: props.selected_id.clone(),
                            theme: props.theme.clone(),
                            depth: props.depth,
                            is_expanded,
                            has_children,
                        },
                        cx,
                    )
                    .into_any_element(),
                );
            }

            if is_expanded {
                elements.extend(
                    self.render_folder_tree(
                        FolderTreeProps {
                            folders: props.folders,
                            parent_id: Some(id),
                            depth: props.depth + 1,
                            selected_id: props.selected_id.clone(),
                            theme: props.theme.clone(),
                            renaming: props.renaming,
                        },
                        cx,
                    )
                    .into_iter()
                    .map(gpui::IntoElement::into_any_element),
                );
            }
        }
        elements
    }

    fn render_folder_item(
        &self,
        props: FolderItemProps,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = props.selected_id.as_ref() == Some(&props.folder.id);
        let folder_id_clone = props.folder.id.clone();
        let folder_id_right = props.folder.id.clone();
        let folder_id_drop = props.folder.id.clone();
        let folder_id_folder_drop = props.folder.id.clone();
        let folder_id_drag = props.folder.id.clone();
        let folder_name_drag = props.folder.name.clone();

        let icon = if is_selected {
            IconName::FolderOpen
        } else {
            IconName::Folder
        };
        let chevron = if props.has_children {
            if props.is_expanded {
                Some(IconName::ChevronDown)
            } else {
                Some(IconName::ChevronRight)
            }
        } else {
            None
        };

        let parent = self.parent_view.clone();

        div()
            .id(SharedString::from(format!(
                "folder-item-wrapper-{}",
                props.folder.id
            )))
            // 拖放支持：当文献拖放到文件夹时
            .on_drop(cx.listener({
                let folder_id = folder_id_drop.clone();
                move |this, drag_info: &LiteratureDragInfo, _, cx| {
                    info!(
                        "拖放文献到文件夹: {} 篇文献 -> {}",
                        drag_info.count(),
                        folder_id
                    );
                    for lit_id in &drag_info.literature_ids {
                        if let Err(e) = this.app.add_literature_to_folder(lit_id, &folder_id) {
                            error!("添加文献到文件夹失败: {e}");
                        }
                    }
                    cx.notify();
                }
            }))
            // 拖放支持：当文件夹拖放到另一个文件夹时 (嵌套移动)
            .on_drop(cx.listener({
                let target_folder_id = folder_id_folder_drop;
                move |this, drag_info: &FolderDragInfo, _, cx| {
                    let source_folder_id = &drag_info.folder_id;

                    // 1. 不能移动到自己内部
                    if source_folder_id == &target_folder_id {
                        return;
                    }

                    // 2. 检查循环引用 (目标文件夹不能是源文件夹的子文件夹)
                    let data = this.data_store.read(cx);
                    let is_descendant = {
                        let mut current_id = Some(target_folder_id.clone());
                        let mut found = false;

                        // 向上遍历 target 的父级，看是否会遇到 source
                        while let Some(cid) = current_id {
                            if cid == *source_folder_id {
                                found = true;
                                break;
                            }
                            // 查找当前节点的父节点
                            if let Some(folder) = data.folders.iter().find(|f| f.id == cid) {
                                current_id = folder.parent_id.clone();
                            } else {
                                break;
                            }
                        }
                        found
                    };

                    if is_descendant {
                        warn!("无法移动文件夹: 目标是源的子文件夹");
                        return;
                    }

                    // 3. 检查是否已经是父级 (避免无意义的操作)
                    let is_already_parent = data
                        .folders
                        .iter()
                        .find(|f| f.id == *source_folder_id)
                        .is_some_and(|f| f.parent_id.as_ref() == Some(&target_folder_id));

                    if is_already_parent {
                        return;
                    }

                    info!("移动文件夹 {source_folder_id} -> {target_folder_id}");
                    let _ = this.app.folder_service.move_folder(
                        this.app.as_ref(),
                        source_folder_id,
                        Some(target_folder_id.clone()),
                    );
                    cx.notify();
                }
            }))
            // 拖拽悬停样式 (文献)
            .drag_over::<LiteratureDragInfo>({
                let theme = props.theme.clone();
                move |style, _, _, _| {
                    style
                        .bg(theme.primary.opacity(0.15))
                        .border_1()
                        .border_color(theme.primary)
                        .rounded_md()
                }
            })
            // 拖拽悬停样式 (文件夹)
            .drag_over::<FolderDragInfo>({
                let theme = props.theme.clone();
                move |style, _, _, _| {
                    style
                        .bg(theme.primary.opacity(0.15))
                        .border_1()
                        .border_color(theme.primary)
                        .rounded_md()
                }
            })
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let folder_id = folder_id_right.clone();
                    let parent = parent.clone();
                    move |this, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        // 1. 先选中文件夹
                        this.select_folder(folder_id.clone(), cx);
                        // 2. 再显示菜单
                        if let Some(mw) = parent.upgrade() {
                            mw.update(cx, |mw, cx| {
                                mw.show_context_menu(
                                    event.position,
                                    ContextMenuType::Folder(Some(folder_id.clone())),
                                    cx,
                                );
                            });
                        }
                    }
                }),
            )
            .child(
                div()
                    .id(SharedString::from(format!(
                        "folder-item-inner-{}",
                        props.folder.id
                    )))
                    .on_drag(
                        FolderDragInfo::new(folder_id_drag, folder_name_drag),
                        |drag_info, _point, _window, cx| {
                            cx.new(|_| {
                                drag_info
                                    .clone()
                                    .with_position(Point::new(px(0.0), px(0.0)))
                            })
                        },
                    )
                    .flex()
                    .items_center()
                    .px_3()
                    .py_0p5()
                    .pl(rems(1.0 * props.depth as f32 + 0.25))
                    .mx_2()
                    .rounded_md()
                    .when(is_selected, |s| {
                        s.bg(props.theme.primary.opacity(0.1))
                            .text_color(props.theme.primary)
                    })
                    .when(!is_selected, |s| s.hover(|s| s.bg(props.theme.muted)))
                    .on_click(cx.listener({
                        let folder_id = folder_id_clone.clone();
                        move |this: &mut Self, event: &gpui::ClickEvent, _, cx| {
                            if event.click_count() == 2 {
                                this.toggle_folder_expansion(folder_id.clone());
                            } else {
                                this.select_folder(folder_id.clone(), cx);
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
                                            .id(SharedString::from(format!(
                                                "folder-item-chevron-{}",
                                                props.folder.id
                                            )))
                                            .w(rems(0.75))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor_pointer()
                                            .on_click(cx.listener({
                                                let folder_id = folder_id_clone.clone();
                                                move |this: &mut Self, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.toggle_folder_expansion(folder_id.clone());
                                                    cx.notify();
                                                }
                                            }))
                                            .children(chevron.map(|c| {
                                                Icon::new(c).xsmall().text_color(if is_selected {
                                                    props.theme.primary
                                                } else {
                                                    props.theme.muted_foreground
                                                })
                                            })),
                                    )
                                    .child(Icon::new(icon).small().text_color(if is_selected {
                                        props.theme.primary
                                    } else {
                                        props.theme.foreground
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(if is_selected {
                                                props.theme.primary
                                            } else {
                                                props.theme.foreground
                                            })
                                            .child(props.folder.name.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(if is_selected {
                                        props.theme.primary.opacity(0.8)
                                    } else {
                                        props.theme.muted_foreground
                                    })
                                    .child(props.folder.literature_count.to_string()),
                            ),
                    ),
            )
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
                s.bg(theme.primary.opacity(0.1)).text_color(theme.primary)
            })
            .when(!is_selected, |s| s.hover(|s| s.bg(theme.secondary)))
            .on_mouse_down(MouseButton::Right, {
                let tag_id = tag_id_right.clone();
                move |event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    if let Some(mw) = parent.upgrade() {
                        mw.update(cx, |mw, cx| {
                            mw.show_context_menu(
                                event.position,
                                ContextMenuType::Tag(Some(tag_id.clone())),
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
                        theme.primary
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
        let (selected_folder_id, selected_tag_id, view_mode) = (
            ui.selected_folder_id.clone(),
            ui.selected_tag_id.clone(),
            ui.view_mode,
        );
        let lang = self.app.current_language();

        // 按名称排序标签
        tags.sort_by(|a, b| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()));

        let parent_view = self.parent_view.clone();
        let theme = cx.theme().clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .bg(cx.theme().muted)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            .relative()
            .when(cfg!(target_os = "macos"), |this| this.pt(rems(3.0)))
            .when(!cfg!(target_os = "macos"), |this| this.pt(rems(2.0)))
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
            // Windows: 顶部拖动区域
            .when(!cfg!(target_os = "macos"), |this| {
                this.child(
                    div()
                        .h(rems(2.0))
                        .w_full()
                        .absolute()
                        .top_0()
                        .left_0()
                        .window_control_area(WindowControlArea::Drag)
                )
            })
            .child(
                h_flex()
                    .px_5()
                    .pb_3()
                    .gap_4()
                    .child(
                        div()
                            .id("tab-library")
                            .cursor_pointer()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if view_mode == AppViewMode::Library {
                                cx.theme().sidebar_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(t(I18nKey::Library, lang))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let parent = this.parent_view.clone();
                                let _ = parent.update(cx, |mw, mw_cx| {
                                    mw.set_view_mode(AppViewMode::Library, mw_cx);
                                });
                            })),
                    )
                    .child(
                        div()
                            .id("tab-subscription")
                            .cursor_pointer()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(if view_mode == AppViewMode::Subscription {
                                cx.theme().sidebar_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(t(I18nKey::Subscription, lang))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let parent = this.parent_view.clone();
                                let _ = parent.update(cx, |mw, mw_cx| {
                                    mw.set_view_mode(AppViewMode::Subscription, mw_cx);
                                });
                            })),
                    ),
            )
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
                    .flex_grow()
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
                            .overflow_y_scroll()
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
                                let _ = this.app.folder_service.move_folder(
                                    this.app.as_ref(),
                                    source_folder_id,
                                    None
                                );
                                cx.notify();
                            }))
                            // 拖拽悬停样式 (全局区域)
                            .drag_over::<FolderDragInfo>({
                                let theme = theme.clone();
                                move |style, _, _, _| {
                                    style
                                        .bg(theme.primary.opacity(0.05))
                                }
                            })
                            .on_mouse_down(MouseButton::Right, move |event: &MouseDownEvent, _, cx| {
                                // 空白区域触发“新建文件夹”菜单
                                if let Some(mw) = parent.upgrade() {
                                    mw.update(cx, |mw, cx| {
                                        mw.show_context_menu(
                                            event.position,
                                            ContextMenuType::Folder(None),
                                            cx,
                                        );
                                    });
                                }
                            })
                            .children(self.render_folder_tree(
                                FolderTreeProps {
                                    folders: &folders,
                                    parent_id: None,
                                    depth: 0,
                                    selected_id: selected_folder_id.clone(),
                                    theme: theme.clone(),
                                    renaming: self.renaming.as_ref(),
                                },
                                cx,
                            ))
                            .child(div().h(rems(6.25)).w_full().flex_shrink_0())
                    })
                    // 2. 标签容器 (固定在底部，位于开发按钮上方)
                    .child(
                        div()
                            .id("tag-container")
                            .flex()
                            .flex_col()
                            .border_t_1()
                            .border_color(theme.border.opacity(0.5))
                            .bg(theme.background)
                            // 标签 Header
                            .child(
                                h_flex()
                                    .px_3()
                                    .py_2()
                                    .justify_between()
                                    .items_center()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.muted.opacity(0.5)))
                                    .child(
                                        div()
                                            .id("tags-header-toggle")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.toggle_tags_expansion(cx);
                                            }))
                                            .child(
                                                h_flex()
                                                    .gap_1()
                                                    .items_center()
                                                    .child(
                                                        Icon::new(if self.tags_expanded { IconName::ChevronDown } else { IconName::ChevronRight })
                                                            .xsmall()
                                                            .text_color(theme.muted_foreground)
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(theme.muted_foreground)
                                                            .child(t(I18nKey::Tags, lang))
                                                    )
                                            )
                                    )
                                    .child(
                                        Button::new("add-tag")
                                            .icon(IconName::Plus)
                                            .ghost()
                                            .xsmall()
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.add_tag(window, cx);
                                            }))
                                    )
                            )
                            // 标签列表内容 (展开时显示，带独立滚动)
                            .when(self.tags_expanded, |this| {
                                let theme = theme.clone();
                                this.child(
                                    div()
                                        .id("tag-scroll-list")
                                        .flex()
                                        .flex_row()
                                        .flex_wrap()
                                        .gap_x_2()
                                        .gap_y_1()
                                        .p_3()
                                        .max_h(rems(12.5)) // 设置最大高度，防止标签过多
                                        .overflow_y_scroll()
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
                                                        .child(Input::new(input_state))
                                                        .into_any_element();
                                            }
                                            self.render_tag_item(tag, selected_tag_id.as_ref(), &theme, cx).into_any_element()
                                        }))
                                )
                            })
                    )
            })
            .child(
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(theme.border.opacity(0.5))
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
                                .text_color(gpui::hsla(0.08, 0.9, 0.5, 1.0)),
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

struct FolderItemProps {
    folder: Arc<Folder>,
    selected_id: Option<String>,
    theme: Theme,
    depth: usize,
    is_expanded: bool,
    has_children: bool,
}
struct FolderTreeProps<'a> {
    folders: &'a [Arc<Folder>],
    parent_id: Option<String>,
    depth: usize,
    selected_id: Option<String>,
    theme: Theme,
    renaming: Option<&'a (String, Entity<InputState>)>,
}
struct StaticItemProps {
    icon_builder: Box<dyn Fn(Hsla) -> AnyElement>,
    text: String,
    count: String,
    is_selected: bool,
    id: String,
    theme: Theme,
}
