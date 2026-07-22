use super::{FetchSource, MainWindow};

use super::types::BatchSource;
use crate::RUNTIME;
use crate::notification_bus::show_notification;
use anyhow::{Error, anyhow};
use components::IconName;
use gpui::anchored;
use gpui::prelude::*;
use gpui::{App, AsyncApp, Hsla, PathPromptOptions, Pixels, Point, Window, px};
use gpui::{ClipboardItem, SharedString, div, rems};
use gpui_component::notification::NotificationType;
use gpui_component::{
    ActiveTheme, Colorize, Icon, h_flex,
    menu::{PopupMenu, PopupMenuItem},
    v_flex,
};
use i18n::{I18nKey, Language, t, tf};
use log::error;
use models::{Folder, FolderType, Literature};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parser::export::ExportFormat;
use services::app::MainApp;
use services::feed::SubscriptionRefreshResult;

/// 构造"破坏性操作"右键菜单项：红色文本 + 红色图标。
///
/// `PopupMenuItem` 本身不暴露文本颜色方法，故用 `element()` 自绘内容，
/// 并通过 `.icon()` 传入已着色的红色 `Icon` 复用默认左图标槽位（保证对齐）。
/// 返回的是普通 `PopupMenuItem`，调用处照常链式 `.on_click(...)` 即可。
fn danger_menu_item(danger: Hsla, label: impl Into<SharedString>, icon: IconName) -> PopupMenuItem {
    let label = label.into();
    PopupMenuItem::element(move |_window, cx| {
        div().text_color(cx.theme().danger).child(label.clone())
    })
    .icon(Icon::new(icon).text_color(danger))
}

/// 从文件夹列表构建层级映射：`name_map`(id->显示名) 与 `children_map`(父id->子id列表)。
/// 仅纳入用户自定义文件夹（`FolderType::Custom`）。
fn build_folder_maps(
    folders: &[Arc<Folder>],
) -> (
    Arc<HashMap<String, String>>,
    Arc<HashMap<Option<String>, Vec<String>>>,
) {
    let mut name_map: HashMap<String, String> = HashMap::new();
    let mut children_map: HashMap<Option<String>, Vec<String>> = HashMap::new();
    for f in folders {
        if f.folder_type != FolderType::Custom {
            continue;
        }
        name_map.insert(f.id.clone(), f.name.clone());
        children_map
            .entry(f.parent_id.clone())
            .or_default()
            .push(f.id.clone());
    }
    // 子文件夹按名称排序，保证菜单展示稳定
    for children in children_map.values_mut() {
        children.sort_by(|a, b| {
            name_map
                .get(a)
                .map(|s| s.to_lowercase())
                .cmp(&name_map.get(b).map(|s| s.to_lowercase()))
        });
    }
    (Arc::new(name_map), Arc::new(children_map))
}

/// 递归把文件夹树渲染为嵌套 `PopupMenu`：
/// - 叶子文件夹 → 直接可点击项（点击 = 加入该文件夹）
/// - 有子文件夹的父文件夹 → `submenu`，其首项为文件夹本名（点击 = 加入该文件夹），下挂子文件夹
///
/// 层级不限，hover 实时展开。父文件夹“既能加入又能展开”通过“子菜单首项用本名”实现，
/// 避免方案 B 的“添加到『X』”丑前缀，也无需自定义菜单组件。
fn build_folder_level(
    mut m: PopupMenu,
    parent_id: Option<String>,
    name_map: &Arc<HashMap<String, String>>,
    children_map: &Arc<HashMap<Option<String>, Vec<String>>>,
    on_select: &Arc<dyn Fn(&str, &mut Window, &mut App) + 'static>,
    window: &mut Window,
    cx: &mut gpui::Context<PopupMenu>,
) -> PopupMenu {
    if let Some(children) = children_map.get(&parent_id) {
        for child_id in children {
            let name = name_map.get(child_id).cloned().unwrap_or_default();
            let has_children = children_map
                .get(&Some(child_id.clone()))
                .map(|c| !c.is_empty())
                .unwrap_or(false);
            if has_children {
                let child_name = name.clone();
                let name_map_c = name_map.clone();
                let children_map_c = children_map.clone();
                let on_select_c = on_select.clone();
                let child_id_c = child_id.clone();
                m = m.submenu(child_name, window, cx, move |mut sub, window, cx| {
                    // 首项：加入本文件夹（用文件夹本名，无丑前缀）
                    let fid2 = child_id_c.clone();
                    let on_select2 = on_select_c.clone();
                    sub = sub.item(PopupMenuItem::new(name.clone()).on_click(
                        move |_, window, cx| {
                            on_select2(&fid2, window, cx);
                        },
                    ));
                    // 递归子文件夹
                    sub = build_folder_level(
                        sub,
                        Some(child_id_c.clone()),
                        &name_map_c,
                        &children_map_c,
                        &on_select_c,
                        window,
                        cx,
                    );
                    sub
                });
            } else {
                let fid = child_id.clone();
                let on_select_self = on_select.clone();
                let self_item = PopupMenuItem::new(name).on_click(move |_, window, cx| {
                    on_select_self(&fid, window, cx);
                });
                m = m.item(self_item);
            }
        }
    }
    m
}

/// 按指定格式（BibTeX / IEEE / 爱思微尔）生成选中文献的引用文本，
/// 复制到剪切板并弹「已复制到剪切板」通知。空选择 / 生成失败分别给错误通知。
fn copy_citation(
    app: &Arc<MainApp>,
    ids: &HashSet<String>,
    format: ExportFormat,
    lang: Language,
    cx: &mut App,
) {
    let lits: Vec<Literature> = app
        .db
        .get_all_literatures()
        .unwrap_or_default()
        .into_iter()
        .filter(|l| ids.contains(&l.id))
        .collect();
    match app.export_manager.export_to_string(format, &lits) {
        Ok(text) if !text.trim().is_empty() => {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            show_notification(
                NotificationType::Success,
                t(I18nKey::CopiedToClipboard, lang),
                cx,
            );
        }
        Ok(_) => show_notification(
            NotificationType::Error,
            t(I18nKey::NoLiteratureSelectedForCitation, lang),
            cx,
        ),
        Err(e) => show_notification(
            NotificationType::Error,
            format!("{}: {e}", t(I18nKey::CitationError, lang)),
            cx,
        ),
    }
}

/// 右键菜单类型
#[derive(Clone, Debug)]
pub enum ContextMenuType {
    Folder(Option<String>),       // 文件夹ID，None表示空白处
    Tag(Option<String>),          // 标签ID，None表示空白处
    Subscription(Option<String>), // 订阅ID，None表示空白处
    SubscriptionAll,              // 「所有订阅」行右键（更新所有订阅）
    SubscriptionItem(String),     // 订阅条目ID
    Literature(String),           // 文献ID
    Attachment(String),           // 附件ID
}

impl MainWindow {
    /// 关闭所有活动的右键菜单和子菜单
    pub fn close_menus(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        cx.notify();
    }

    pub fn show_context_menu(
        &mut self,
        pos: Point<Pixels>,
        menu_type: ContextMenuType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu_view = self.build_context_menu(menu_type, window, cx);
        self.context_menu = Some((pos, menu_view));
        cx.notify();
    }

    pub(super) fn render_global_context_menu(
        &self,
        _cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let (pos, menu_view) = self.context_menu.as_ref()?;
        let mut pos = *pos;
        let menu_width = px(160.0);

        // 边界检测：防止菜单超出右侧
        if pos.x + menu_width > self.current_window_width {
            pos.x -= menu_width;
        }

        // 使用 gpui 的 anchored() 直接锚定原生 PopupMenu 视图实体
        let element = anchored().position(pos).child(menu_view.clone());

        Some(element)
    }

    /// 构建原生 PopupMenu 实体与级联二级菜单
    pub fn build_context_menu(
        &self,
        menu_type: ContextMenuType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Entity<PopupMenu> {
        let lang = self.app.current_language();
        let this_weak = cx.weak_entity();

        // ── 提前在 PopupMenu::build 之前读取所有需要 cx/self 的数据 ──
        // 这样闭包内就不需要再借用 cx，彻底避免二次借用 panic

        let current_selected_folder = cx
            .global::<crate::services::ui_state::UiState>()
            .selected_folder_id
            .clone();

        // SubscriptionItem 分支所需数据
        let sub_item_state: Option<(bool, bool)> =
            if let ContextMenuType::SubscriptionItem(ref sub_id) = menu_type {
                let data = self.data_store.read(cx);
                Some(
                    if let Some(s) = data.feed_items.iter().find(|s| &s.id == sub_id) {
                        (s.is_read, s.is_added_to_library)
                    } else {
                        (false, false)
                    },
                )
            } else {
                None
            };

        // SubscriptionItem 分支：预取自定义文件夹层级映射（用于「添加到文献库」多级子菜单）
        let (sub_name_map, sub_children_map): (
            Arc<HashMap<String, String>>,
            Arc<HashMap<Option<String>, Vec<String>>>,
        ) = if let ContextMenuType::SubscriptionItem(_) = menu_type {
            let data = self.data_store.read(cx);
            build_folder_maps(&data.folders)
        } else {
            (Arc::new(HashMap::new()), Arc::new(HashMap::new()))
        };

        // Attachment 分支所需数据
        let attachment_lit_data: Option<(String, bool, String)> =
            if let ContextMenuType::Attachment(ref att_id) = menu_type {
                let data = self.data_store.read(cx);
                data.literatures.iter().find_map(|l| {
                    l.attachments
                        .iter()
                        .find(|a| &a.id == att_id)
                        .map(|a| (l.id.clone(), a.is_main, a.file_path.clone()))
                })
            } else {
                None
            };

        // Literature 分支所需数据
        let literature_prefetch: Option<(
            usize,
            std::collections::HashSet<String>,
            bool,
            Option<models::Literature>,
            (
                Arc<HashMap<String, String>>,
                Arc<HashMap<Option<String>, Vec<String>>>,
            ),
        )> = if let ContextMenuType::Literature(ref lit_id) = menu_type {
            let ui = cx.global::<crate::services::ui_state::UiState>();
            let selected_count = ui.selected_literature_ids.len();
            let selected_ids = ui.selected_literature_ids.clone();
            let _ = ui; // 释放不可变借用

            let data = self.data_store.read(cx);
            let lit = data
                .literatures
                .iter()
                .find(|l| l.id == *lit_id)
                .map(|l| (**l).clone());
            let in_trash = lit
                .as_ref()
                .is_some_and(|l| l.folder_ids.contains(&"trash".to_string()));

            // 自定义文件夹层级映射
            let (custom_name_map, custom_children_map) = build_folder_maps(&data.folders);

            Some((
                selected_count,
                selected_ids,
                in_trash,
                lit,
                (custom_name_map, custom_children_map),
            ))
        } else {
            None
        };

        let window_ref: &mut Window = window;
        let app_ref: &mut App = cx;

        // 统一在外部进行 PopupMenu::build 构造，使用显式的 Window 和 App 引用
        PopupMenu::build(window_ref, app_ref, move |mut menu, window, cx| {
            match menu_type {
                ContextMenuType::Folder(target_id) => {
                    if target_id.as_deref() == Some("trash") {
                        let this_weak_clone = this_weak.clone();
                        menu = menu.item(
                            danger_menu_item(
                                cx.theme().danger,
                                t(I18nKey::EmptyTrash, lang),
                                IconName::Trash,
                            )
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.handle_empty_trash(cx);
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                        );
                    } else {
                        // 1. 新建子文件夹
                        let this_weak_clone = this_weak.clone();
                        let target_id_clone = target_id.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(I18nKey::NewFolder, lang))
                                .icon(Icon::new(IconName::Plus))
                                .on_click(move |_, window, cx| {
                                    if let Some(this) = this_weak_clone.upgrade() {
                                        this.update(cx, |this, cx| {
                                            this.literature_panel.update(cx, |panel, cx| {
                                                panel.add_folder(
                                                    target_id_clone.clone(),
                                                    window,
                                                    cx,
                                                );
                                            });
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                        );

                        // 2. 文件夹重命名与删除 (仅限非系统默认文件夹)
                        if let Some(fid) = target_id {
                            let is_system =
                                fid == "all" || fid == "uncategorized" || fid == "trash";
                            if !is_system {
                                let this_weak_clone = this_weak.clone();
                                let fid_rename = fid.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(t(I18nKey::Rename, lang))
                                        .icon(Icon::new(IconName::Edit))
                                        .on_click(move |_, window, cx| {
                                            if let Some(this) = this_weak_clone.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    this.literature_panel.update(
                                                        cx,
                                                        |panel, cx| {
                                                            panel.start_rename(
                                                                fid_rename.clone(),
                                                                false,
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        }),
                                );

                                let this_weak_clone = this_weak.clone();
                                let fid_delete = fid.clone();
                                menu = menu.item(
                                    danger_menu_item(
                                        cx.theme().danger,
                                        t(I18nKey::Delete, lang),
                                        IconName::Trash,
                                    )
                                    .on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_clone.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    this.literature_panel.update(
                                                        cx,
                                                        |panel, cx| {
                                                            panel.delete_folder(
                                                                fid_delete.clone(),
                                                                cx,
                                                            );
                                                        },
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ),
                                );
                            }
                        }
                    }
                }
                ContextMenuType::Tag(target_id) => {
                    // 标签重命名与删除
                    if let Some(tid) = target_id {
                        let this_weak_clone = this_weak.clone();
                        let tid_rename = tid.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(I18nKey::Rename, lang))
                                .icon(Icon::new(IconName::Edit))
                                .on_click(move |_, window, cx| {
                                    if let Some(this) = this_weak_clone.upgrade() {
                                        this.update(cx, |this, cx| {
                                            this.literature_panel.update(cx, |panel, cx| {
                                                panel.start_tag_rename(
                                                    tid_rename.clone(),
                                                    false,
                                                    window,
                                                    cx,
                                                );
                                            });
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                        );

                        // 颜色选择
                        menu = menu.separator();
                        let this_weak_clone = this_weak.clone();
                        let tid_color = tid.clone();
                        menu = menu.item(PopupMenuItem::element(move |_window, cx| {
                            let this_weak_inner = this_weak_clone.clone();
                            let tid_inner = tid_color.clone();

                            let (tag_name, current_color) =
                                if let Some(this) = this_weak_inner.upgrade() {
                                    this.update(cx, |this, cx| {
                                        let data = this.data_store.read(cx);
                                        data.tags
                                            .iter()
                                            .find(|(t, _)| t.id == tid_inner)
                                            .map(|(t, _)| (t.name.clone(), t.color.clone()))
                                            .unwrap_or_default()
                                    })
                                } else {
                                    (String::new(), String::new())
                                };

                            let tag_colors_ref = models::tag::TAG_COLORS;
                            let tag_colors: Vec<&str> =
                                tag_colors_ref.iter().map(|(_, hex)| *hex).collect();

                            let this_weak_inner2 = this_weak_inner.clone();
                            let tid_inner2 = tid_inner.clone();
                            let tag_name_inner2 = tag_name.clone();
                            let active_border_color = cx.theme().foreground;

                            v_flex().mx(gpui::px(-8.0)).px_2().py_1().gap_1().children(
                                tag_colors
                                    .chunks(5)
                                    .map(move |chunk| {
                                        let chunk = chunk.to_vec();
                                        let current_color = current_color.clone();
                                        let tag_name = tag_name_inner2.clone();
                                        let tid = tid_inner2.clone();
                                        let this_weak = this_weak_inner2.clone();
                                        let active_border = active_border_color;

                                        h_flex().w_full().justify_around().gap_1().py_1().children(
                                            chunk
                                                .iter()
                                                .map(move |&color_hex| {
                                                    let color_hex = color_hex.to_string();
                                                    let is_active = current_color == color_hex;
                                                    let color_hex_clone = color_hex.clone();
                                                    let tid_clone = tid.clone();
                                                    let tag_name_clone = tag_name.clone();
                                                    let this_weak_click = this_weak.clone();

                                                    let color = gpui::Hsla::parse_hex(&color_hex)
                                                        .unwrap_or(gpui::red());

                                                    div()
                                                        .id(SharedString::from(format!(
                                                            "color-{}",
                                                            color_hex
                                                        )))
                                                        .size(rems(1.0))
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .rounded_full()
                                                        .cursor_pointer()
                                                        .when(is_active, |this| {
                                                            this.border_2()
                                                                .border_color(active_border)
                                                        })
                                                        .child(
                                                            div()
                                                                .size(rems(0.6))
                                                                .rounded_full()
                                                                .bg(color),
                                                        )
                                                        .on_mouse_down(
                                                            gpui::MouseButton::Left,
                                                            move |_, _, cx| {
                                                                cx.stop_propagation();
                                                                if let Some(this) =
                                                                    this_weak_click.upgrade()
                                                                {
                                                                    this.update(cx, |this, cx| {
                                                                        let _ = this
                                                                            .app
                                                                            .tag_service
                                                                            .update_tag(
                                                                                &this.app.db,
                                                                                || this.app.notify_data_changed(),
                                                                                &tid_clone,
                                                                                &tag_name_clone,
                                                                                &color_hex_clone,
                                                                            );
                                                                        this.close_menus(cx);
                                                                    });
                                                                }
                                                            },
                                                        )
                                                })
                                                .collect::<Vec<_>>(),
                                        )
                                    })
                                    .collect::<Vec<_>>(),
                            )
                        }));
                        menu = menu.separator();

                        let this_weak_clone = this_weak.clone();
                        let tid_delete = tid.clone();
                        menu = menu.item(
                            danger_menu_item(
                                cx.theme().danger,
                                t(I18nKey::Delete, lang),
                                IconName::Trash,
                            )
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    let app = this.read(cx).app.clone();
                                    let id = tid_delete.clone();
                                    let _ = app.tag_service.delete_tag(
                                        &app.db,
                                        || app.notify_data_changed(),
                                        &id,
                                    );
                                    this.update(cx, |this, cx| {
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                        );
                    }
                }
                ContextMenuType::Subscription(target_id) => {
                    // 1. 订阅编辑与删除（新建订阅入口已移至顶部工具栏/菜单，此处不再重复）
                    if let Some(sid) = target_id {
                        // 2.1 立即更新订阅
                        let this_weak_clone = this_weak.clone();
                        let sid_update = sid.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(I18nKey::UpdateSubscription, lang))
                                .icon(Icon::new(IconName::RotateCw))
                                .on_click(move |_, _window, cx| {
                                    if let Some(this) = this_weak_clone.upgrade() {
                                        let app = this.read(cx).app.clone();
                                        if let Err(e) = app.refresh_feed(&sid_update) {
                                            error!("手动刷新订阅失败: {e}");
                                        }
                                        this.update(cx, |this, cx| {
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                        );

                        // 2.2 编辑
                        let this_weak_clone = this_weak.clone();
                        let sid_edit = sid.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(I18nKey::Edit, lang))
                                .icon(Icon::new(IconName::Edit))
                                .on_click(move |_, window, cx| {
                                    if let Some(this) = this_weak_clone.upgrade() {
                                        this.update(cx, |this, cx| {
                                            this.open_edit_subscription_modal(
                                                sid_edit.clone(),
                                                window,
                                                cx,
                                            );
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                        );

                        // 2.3 删除
                        let this_weak_clone = this_weak.clone();
                        let sid_delete = sid.clone();
                        menu = menu.item(
                            danger_menu_item(
                                cx.theme().danger,
                                t(I18nKey::Delete, lang),
                                IconName::Trash,
                            )
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    let app = this.read(cx).app.clone();
                                    let _ = app.delete_feed(&sid_delete);
                                    this.update(cx, |this, cx| {
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                        );
                    }
                }
                ContextMenuType::SubscriptionAll => {
                    let this_weak_clone = this_weak.clone();
                    menu = menu.item(
                        PopupMenuItem::new(t(I18nKey::UpdateAllSubscriptions, lang))
                            .icon(Icon::new(IconName::RotateCw))
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    let app = this.read(cx).app.clone();
                                    match app.refresh_all_subscriptions() {
                                        Ok(rx) => {
                                            let _ = cx.spawn(async move |cx: &mut AsyncApp| {
                                                let mut rx = rx;
                                                while let Some(r) = rx.recv().await {
                                                    cx.update(|app| match r {
                                                        SubscriptionRefreshResult::Ok { name } => {
                                                            show_notification(
                                                                NotificationType::Success,
                                                                tf(
                                                                    I18nKey::SubscriptionUpdated,
                                                                    lang,
                                                                    &[name.as_str()],
                                                                ),
                                                                app,
                                                            );
                                                        }
                                                        SubscriptionRefreshResult::Err { name, error } => {
                                                            show_notification(
                                                                NotificationType::Error,
                                                                tf(
                                                                    I18nKey::SubscriptionUpdateFailed,
                                                                    lang,
                                                                    &[name.as_str(), error.as_str()],
                                                                ),
                                                                app,
                                                            );
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                        Err(e) => {
                                            error!("手动刷新所有订阅失败: {e}");
                                        }
                                    }
                                    this.update(cx, |this, cx| {
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                    );
                }
                ContextMenuType::SubscriptionItem(sub_id) => {
                    let (is_read, is_added) = sub_item_state.unwrap_or((false, false));

                    // 1. 添加到文献库（二级菜单：未分类 + 各文件夹）
                    if !is_added {
                        let this_weak_clone = this_weak.clone();
                        let sub_id_add = sub_id.clone();

                        let add_library_submenu =
                            PopupMenu::build(window, cx, move |mut m, window, cx| {
                                // 1.1 文献库（原未分类）：仅加入文献库，不指定文件夹
                                let this_weak_uncat = this_weak_clone.clone();
                                let sub_id_uncat = sub_id_add.clone();
                                m = m.item(PopupMenuItem::new(t(I18nKey::Library, lang)).on_click(
                                    move |_, _window, cx| {
                                        if let Some(this) = this_weak_uncat.upgrade() {
                                            let id = sub_id_uncat.clone();
                                            let app = this.read(cx).app.clone();
                                            if let Err(e) = app.add_feed_item_to_library(&id) {
                                                error!("添加到文献库失败: {e}");
                                            }
                                            this.update(cx, |this, cx| this.close_menus(cx));
                                        }
                                    },
                                ));
                                // 1.2 各自定义文件夹（多级树，不限层级）
                                let on_select: Arc<dyn Fn(&str, &mut Window, &mut App) + 'static> =
                                    Arc::new({
                                        let this_weak_tree = this_weak_clone.clone();
                                        let sub_id_tree = sub_id_add.clone();
                                        move |folder_id, _window, cx| {
                                            if let Some(this) = this_weak_tree.upgrade() {
                                                let id = sub_id_tree.clone();
                                                let app = this.read(cx).app.clone();
                                                match app.add_feed_item_to_library(&id) {
                                                    Ok(lit_id) => {
                                                        let _ = app.add_literature_to_folder(
                                                            &lit_id, folder_id,
                                                        );
                                                    }
                                                    Err(e) => {
                                                        error!("添加到文献库失败: {e}");
                                                    }
                                                }
                                                this.update(cx, |this, cx| this.close_menus(cx));
                                            }
                                        }
                                    });
                                m = build_folder_level(
                                    m,
                                    None,
                                    &sub_name_map,
                                    &sub_children_map,
                                    &on_select,
                                    window,
                                    cx,
                                );
                                m
                            });

                        menu = menu.item(
                            PopupMenuItem::submenu(
                                t(I18nKey::AddToLibrary, lang),
                                add_library_submenu,
                            )
                            .icon(Icon::new(IconName::Plus)),
                        );
                    }

                    // 2. 标记已读/未读
                    let this_weak_clone = this_weak.clone();
                    let sub_id_read = sub_id.clone();
                    let label = if is_read {
                        t(I18nKey::MarkAsUnread, lang)
                    } else {
                        t(I18nKey::MarkAsRead, lang)
                    };
                    menu = menu.item(
                        PopupMenuItem::new(label)
                            .icon(Icon::new(IconName::Bell))
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    let id = sub_id_read.clone();
                                    let app = this.read(cx).app.clone();
                                    let mut hs = HashSet::new();
                                    hs.insert(id.clone());
                                    if let Err(e) =
                                        app.smart_toggle_feed_items_read(&id, !is_read, &hs)
                                    {
                                        error!("更新已读状态失败: {e}");
                                    }
                                    this.update(cx, |this, cx| {
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                    );
                }
                ContextMenuType::Attachment(att_id) => {
                    // 获取附件对应文献ID以及是否是主文件（已在 build_context_menu 外提前读取）
                    let cached_lit_data = attachment_lit_data.clone();

                    // 1. 更换文件
                    let this_weak_clone = this_weak.clone();
                    let att_id_change = att_id.clone();
                    let cached_lit_data_clone = cached_lit_data.clone();
                    menu = menu.item(
                        PopupMenuItem::new(t(I18nKey::ReplaceFile, lang))
                            .icon(Icon::new(IconName::Edit))
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    let app = this.read(cx).app.clone();
                                    let att_id = att_id_change.clone();
                                    let cached_lit_data = cached_lit_data_clone.clone();

                                    // 弹窗选择文件
                                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                                        files: true,
                                        directories: false,
                                        multiple: false,
                                        prompt: Some(t(I18nKey::SelectNewFile, lang).into()),
                                    });

                                    // 使用 Runtime 开启后台文件导入
                                    RUNTIME.spawn(async move {
                                        if let Ok(Ok(Some(paths))) = receiver.await
                                            && let Some(path) = paths.first().cloned()
                                        {
                                            let result = (|| {
                                                let (lit_id, is_main, _) = cached_lit_data
                                                    .ok_or_else(|| {
                                                        anyhow!("Literature not found")
                                                    })?;

                                                if is_main {
                                                    app.import_file_to_literature(
                                                        &lit_id, &path, true,
                                                    )?;
                                                } else {
                                                    app.delete_attachment_file(&att_id)?;
                                                    app.import_file_to_literature(
                                                        &lit_id, &path, false,
                                                    )?;
                                                }
                                                Ok::<(), Error>(())
                                            })(
                                            );

                                            if let Err(e) = result {
                                                error!("更换文件失败: {e}");
                                            }
                                        }
                                    });

                                    this.update(cx, |this, cx| {
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                    );

                    // 1.5 在 Finder/Explorer 中显示
                    if let Some((_, _, ref file_path)) = cached_lit_data {
                        let path_clone = file_path.clone();
                        let this_weak_ret = this_weak.clone();
                        menu = menu.item(
                            PopupMenuItem::new(t(I18nKey::OpenPath, lang))
                                .icon(Icon::new(IconName::FolderOpen))
                                .on_click(move |_, _window, cx| {
                                    cx.reveal_path(std::path::Path::new(&path_clone));
                                    if let Some(this) = this_weak_ret.upgrade() {
                                        this.update(cx, |this, cx| {
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                        );
                    }

                    // 2. 删除附件
                    let this_weak_clone = this_weak.clone();
                    let att_id_delete = att_id.clone();
                    menu = menu.item(
                        danger_menu_item(
                            cx.theme().danger,
                            t(I18nKey::DeleteFile, lang),
                            IconName::Trash,
                        )
                        .on_click(move |_, _window, cx| {
                            if let Some(this) = this_weak_clone.upgrade() {
                                let att_id = att_id_delete.clone();
                                let app = this.read(cx).app.clone();
                                let _ = app.delete_attachment_file(&att_id);
                                this.update(cx, |this, cx| {
                                    this.close_menus(cx);
                                });
                            }
                        }),
                    );
                }
                ContextMenuType::Literature(lit_id) => {
                    // 使用提前预取的数据，避免闭包内二次借用 cx
                    let (
                        selected_count,
                        selected_ids,
                        in_trash,
                        lit,
                        (custom_name_map, custom_children_map),
                    ) = literature_prefetch.clone().unwrap_or_default();
                    if in_trash {
                        // 1. 还原到
                        let this_weak_clone = this_weak.clone();
                        let lit_id_restore = lit_id.clone();
                        let sel_ids = selected_ids.clone();
                        let custom_nm = custom_name_map.clone();
                        let custom_cm = custom_children_map.clone();

                        let restore_submenu =
                            PopupMenu::build(window, cx, move |mut m, window, cx| {
                                let this_weak_inner = this_weak_clone.clone();
                                let lit_id_inner = lit_id_restore.clone();
                                let sel_ids_inner = sel_ids.clone();
                                m = m.item(
                                    PopupMenuItem::new(t(I18nKey::AllLiterature, lang)).on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    let _ = this.app.smart_restore_literatures(
                                                        &lit_id_inner,
                                                        None,
                                                        &sel_ids_inner,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ),
                                );

                                let on_select: Arc<dyn Fn(&str, &mut Window, &mut App) + 'static> =
                                    Arc::new({
                                        let this_weak_tree = this_weak_clone.clone();
                                        let lit_id_tree = lit_id_restore.clone();
                                        let sel_ids_tree = sel_ids.clone();
                                        move |folder_id, _window, cx| {
                                            if let Some(this) = this_weak_tree.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    let _ = this.app.smart_restore_literatures(
                                                        &lit_id_tree,
                                                        Some(folder_id),
                                                        &sel_ids_tree,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        }
                                    });
                                m = build_folder_level(
                                    m, None, &custom_nm, &custom_cm, &on_select, window, cx,
                                );
                                m
                            });

                        menu = menu.item(
                            PopupMenuItem::submenu(t(I18nKey::RestoreTo, lang), restore_submenu)
                                .icon(Icon::new(IconName::Undo)),
                        );

                        menu = menu.separator();

                        // 2. 永久删除
                        let this_weak_clone = this_weak.clone();
                        let lit_id_delete = lit_id.clone();
                        let sel_ids = selected_ids.clone();
                        menu = menu.item(
                            danger_menu_item(
                                cx.theme().danger,
                                t(I18nKey::PermanentDelete, lang),
                                IconName::Trash,
                            )
                            .on_click(move |_, _window, cx| {
                                if let Some(this) = this_weak_clone.upgrade() {
                                    this.update(cx, |this, cx| {
                                        let _ = this
                                            .app
                                            .smart_delete_literature(&lit_id_delete, &sel_ids);
                                        this.close_menus(cx);
                                    });
                                }
                            }),
                        );
                    } else {
                        // ==================== 第一菜单组：修改、查看等一级菜单 ====================
                        if selected_count <= 1 {
                            // 1. 编辑文献
                            let this_weak_clone = this_weak.clone();
                            let lit_id_edit = lit_id.clone();
                            menu = menu.item(
                                PopupMenuItem::new(t(I18nKey::Edit, lang))
                                    .icon(Icon::new(IconName::Edit))
                                    .on_click(move |_, _window, cx| {
                                        if let Some(this) = this_weak_clone.upgrade() {
                                            this.update(cx, |this, cx| {
                                                this.open_edit_modal(Some(lit_id_edit.clone()), cx);
                                                this.close_menus(cx);
                                            });
                                        }
                                    }),
                            );

                            // 3. 从...获取元数据 (包含 ArXiv / DBLP / DOI / OpenAlex 原生二级子菜单)
                            if let Some(lit) = &lit {
                                let lit_clone = lit.clone();
                                let this_weak_clone = this_weak.clone();
                                let fetch_submenu =
                                    PopupMenu::build(window, cx, move |mut m, _window, _cx| {
                                        // 2.1 ArXiv
                                        let this_weak_inner = this_weak_clone.clone();
                                        let lit_inner = lit_clone.clone();
                                        m = m.item(PopupMenuItem::new("ArXiv").on_click(
                                            move |_, window, cx| {
                                                if let Some(this) = this_weak_inner.upgrade() {
                                                    this.update(cx, |this, cx| {
                                                        let arxiv_id =
                                                            super::utils::extract_arxiv_id(
                                                                &lit_inner,
                                                            );
                                                        if let Some(id) = arxiv_id {
                                                            this.start_fetch_and_compare(
                                                                std::sync::Arc::new(
                                                                    lit_inner.clone(),
                                                                ),
                                                                FetchSource::ArXiv(id),
                                                                window,
                                                                cx,
                                                            );
                                                        } else {
                                                            show_notification(
                                                                NotificationType::Error,
                                                                format!(
                                                                    "{}: {}",
                                                                    t(I18nKey::FetchFailed, lang),
                                                                    t(I18nKey::FetchFailed, lang)
                                                                ),
                                                                cx,
                                                            );
                                                        }
                                                        this.close_menus(cx);
                                                    });
                                                }
                                            },
                                        ));

                                        // 2.2 DBLP
                                        let this_weak_inner = this_weak_clone.clone();
                                        let lit_inner = lit_clone.clone();
                                        m = m.item(PopupMenuItem::new("DBLP").on_click(
                                            move |_, window, cx| {
                                                if let Some(this) = this_weak_inner.upgrade() {
                                                    this.update(cx, |this, cx| {
                                                        if lit_inner.title.is_empty() {
                                                            show_notification(
                                                                NotificationType::Error,
                                                                format!(
                                                                    "{}: {}",
                                                                    t(I18nKey::FetchFailed, lang),
                                                                    t(I18nKey::FetchFailed, lang)
                                                                ),
                                                                cx,
                                                            );
                                                        } else {
                                                            this.start_fetch_and_compare(
                                                                std::sync::Arc::new(
                                                                    lit_inner.clone(),
                                                                ),
                                                                FetchSource::Dblp(
                                                                    lit_inner.title.clone(),
                                                                ),
                                                                window,
                                                                cx,
                                                            );
                                                        }
                                                        this.close_menus(cx);
                                                    });
                                                }
                                            },
                                        ));

                                        // 2.3 DOI
                                        let this_weak_inner = this_weak_clone.clone();
                                        let lit_inner = lit_clone.clone();
                                        m = m.item(PopupMenuItem::new("DOI").on_click(
                                            move |_, window, cx| {
                                                if let Some(this) = this_weak_inner.upgrade() {
                                                    this.update(cx, |this, cx| {
                                                        let doi_opt = lit_inner.doi.clone();
                                                        if let Some(id) = doi_opt {
                                                            this.start_fetch_and_compare(
                                                                std::sync::Arc::new(
                                                                    lit_inner.clone(),
                                                                ),
                                                                FetchSource::Doi(id),
                                                                window,
                                                                cx,
                                                            );
                                                        } else {
                                                            show_notification(
                                                                NotificationType::Error,
                                                                format!(
                                                                    "{}: {}",
                                                                    t(I18nKey::FetchFailed, lang),
                                                                    t(I18nKey::FetchFailed, lang)
                                                                ),
                                                                cx,
                                                            );
                                                        }
                                                        this.close_menus(cx);
                                                    });
                                                }
                                            },
                                        ));

                                        // 2.4 OpenAlex
                                        let this_weak_inner = this_weak_clone.clone();
                                        let lit_inner = lit_clone.clone();
                                        m = m.item(PopupMenuItem::new("OpenAlex").on_click(
                                            move |_, window, cx| {
                                                if let Some(this) = this_weak_inner.upgrade() {
                                                    this.update(cx, |this, cx| {
                                                        let doi_opt = lit_inner.doi.clone();
                                                        if let Some(id) = doi_opt {
                                                            this.start_fetch_and_compare(
                                                                std::sync::Arc::new(
                                                                    lit_inner.clone(),
                                                                ),
                                                                FetchSource::OpenAlexDoi(id),
                                                                window,
                                                                cx,
                                                            );
                                                        } else {
                                                            show_notification(
                                                                NotificationType::Error,
                                                                format!(
                                                                    "{}: {}",
                                                                    t(I18nKey::FetchFailed, lang),
                                                                    t(I18nKey::FetchFailed, lang)
                                                                ),
                                                                cx,
                                                            );
                                                        }
                                                        this.close_menus(cx);
                                                    });
                                                }
                                            },
                                        ));

                                        m
                                    });

                                menu = menu.item(
                                    PopupMenuItem::submenu(
                                        t(I18nKey::FetchFrom, lang),
                                        fetch_submenu,
                                    )
                                    .icon(Icon::new(IconName::Cloud)),
                                );
                            }
                        }

                        // 3. 复制引用 (二级子菜单: BibTeX / IEEE / 爱思微尔)
                        let this_weak_clone = this_weak.clone();
                        let sel_ids = selected_ids.clone();
                        let citation_submenu =
                            PopupMenu::build(window, cx, move |mut m, _window, _cx| {
                                // BibTeX
                                let this_weak_inner = this_weak_clone.clone();
                                let sel_ids_inner = sel_ids.clone();
                                m = m.item(
                                    PopupMenuItem::new(t(I18nKey::CitationBibTeX, lang)).on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    copy_citation(
                                                        &this.app,
                                                        &sel_ids_inner,
                                                        ExportFormat::BibTeX,
                                                        lang,
                                                        cx,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ),
                                );
                                // IEEE
                                let this_weak_inner = this_weak_clone.clone();
                                let sel_ids_inner = sel_ids.clone();
                                m = m.item(
                                    PopupMenuItem::new(t(I18nKey::CitationIeee, lang)).on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    copy_citation(
                                                        &this.app,
                                                        &sel_ids_inner,
                                                        ExportFormat::IEEE,
                                                        lang,
                                                        cx,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ),
                                );
                                // 爱思微尔
                                let this_weak_inner = this_weak_clone.clone();
                                let sel_ids_inner = sel_ids.clone();
                                m = m.item(
                                    PopupMenuItem::new(t(I18nKey::CitationElsevier, lang))
                                        .on_click(move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    copy_citation(
                                                        &this.app,
                                                        &sel_ids_inner,
                                                        ExportFormat::Elsevier,
                                                        lang,
                                                        cx,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        }),
                                );
                                m
                            });

                        menu = menu.item(
                            PopupMenuItem::submenu(
                                t(I18nKey::CopyCitation, lang),
                                citation_submenu,
                            )
                            .icon(Icon::new(IconName::Copy)),
                        );

                        // ==================== 第二菜单组：级联文件夹管理 ====================
                        // 5. 添加到（级联文件夹菜单，多级树，不限层级）
                        if !custom_name_map.is_empty() {
                            let custom_nm = custom_name_map.clone();
                            let custom_cm = custom_children_map.clone();
                            let this_weak_clone = this_weak.clone();
                            let lit_id_add = lit_id.clone();
                            let sel_ids = selected_ids.clone();

                            let on_select: Arc<dyn Fn(&str, &mut Window, &mut App) + 'static> =
                                Arc::new({
                                    let this_weak_tree = this_weak_clone.clone();
                                    let lit_id_tree = lit_id_add.clone();
                                    let sel_ids_tree = sel_ids.clone();
                                    move |folder_id, _window, cx| {
                                        if let Some(this) = this_weak_tree.upgrade() {
                                            this.update(cx, |this, cx| {
                                                let _ = this.app.smart_add_literatures_to_folder(
                                                    &lit_id_tree,
                                                    folder_id,
                                                    &sel_ids_tree,
                                                );
                                                this.close_menus(cx);
                                            });
                                        }
                                    }
                                });

                            let add_submenu =
                                PopupMenu::build(window, cx, move |mut m, window, cx| {
                                    m = build_folder_level(
                                        m, None, &custom_nm, &custom_cm, &on_select, window, cx,
                                    );
                                    m
                                });

                            menu = menu.item(
                                PopupMenuItem::submenu(t(I18nKey::AddTo, lang), add_submenu)
                                    .icon(Icon::new(IconName::Folder)),
                            );
                        }

                        // 6. 从文件夹中移除
                        if let Some(folder_id) = &current_selected_folder
                            && folder_id != "all"
                            && folder_id != "uncategorized"
                            && folder_id != "trash"
                        {
                            let this_weak_clone = this_weak.clone();
                            let lit_id_remove = lit_id.clone();
                            let folder_id_clone = folder_id.clone();
                            let sel_ids = selected_ids.clone();
                            menu = menu.separator();
                            menu = menu.item(
                                danger_menu_item(
                                    cx.theme().danger,
                                    t(I18nKey::RemoveFromFolder, lang),
                                    IconName::FolderOpen,
                                )
                                .on_click(move |_, _window, cx| {
                                    if let Some(this) = this_weak_clone.upgrade() {
                                        this.update(cx, |this, cx| {
                                            let _ = this.app.smart_remove_literatures_from_folder(
                                                &lit_id_remove,
                                                &folder_id_clone,
                                                &sel_ids,
                                            );
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                            );
                        }
                        // ==================== 第三菜单组：删除与批量元数据获取 ====================
                        menu = menu.separator();

                        // 7. 批量获取元数据
                        if selected_count > 1 {
                            let this_weak_clone = this_weak.clone();
                            let sel_ids = selected_ids.clone();
                            let batch_submenu =
                                PopupMenu::build(window, cx, move |mut m, _window, _cx| {
                                    // 7.1 ArXiv 批量
                                    let this_weak_inner = this_weak_clone.clone();
                                    let sel_ids_inner = {
                                        let mut hs = Vec::new();
                                        hs.extend(sel_ids.clone());
                                        hs
                                    };
                                    m = m.item(PopupMenuItem::new("ArXiv").on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    let mut items = Vec::new();
                                                    items.extend(sel_ids_inner.clone());
                                                    this.handle_batch_fetch_metadata(
                                                        items,
                                                        BatchSource::ArXiv,
                                                        cx,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ));

                                    // 7.2 Crossref 批量
                                    let this_weak_inner = this_weak_clone.clone();
                                    let sel_ids_inner = {
                                        let mut hs = Vec::new();
                                        hs.extend(sel_ids.clone());
                                        hs
                                    };
                                    m = m.item(PopupMenuItem::new("Crossref").on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    let mut items = Vec::new();
                                                    items.extend(sel_ids_inner.clone());
                                                    this.handle_batch_fetch_metadata(
                                                        items,
                                                        BatchSource::Doi,
                                                        cx,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ));

                                    // 7.3 OpenAlex 批量
                                    let this_weak_inner = this_weak_clone.clone();
                                    let sel_ids_inner = {
                                        let mut hs = Vec::new();
                                        hs.extend(sel_ids.clone());
                                        hs
                                    };
                                    m = m.item(PopupMenuItem::new("OpenAlex").on_click(
                                        move |_, _window, cx| {
                                            if let Some(this) = this_weak_inner.upgrade() {
                                                this.update(cx, |this, cx| {
                                                    let mut items = Vec::new();
                                                    items.extend(sel_ids_inner.clone());
                                                    this.handle_batch_fetch_metadata(
                                                        items,
                                                        BatchSource::OpenAlex,
                                                        cx,
                                                    );
                                                    this.close_menus(cx);
                                                });
                                            }
                                        },
                                    ));
                                    m
                                });

                            menu = menu.item(
                                PopupMenuItem::submenu(
                                    t(I18nKey::BatchFetchMetadata, lang),
                                    batch_submenu,
                                )
                                .icon(Icon::new(IconName::Cloud)),
                            );
                        }

                        // 8. 删除
                        let this_weak_clone = this_weak.clone();
                        let lit_id_delete = lit_id.clone();
                        let sel_ids = selected_ids.clone();
                        let delete_label = t(I18nKey::Delete, lang);
                        menu = menu.item(
                            danger_menu_item(cx.theme().danger, delete_label, IconName::Trash)
                                .on_click(move |_, _window, cx| {
                                    if let Some(this) = this_weak_clone.upgrade() {
                                        this.update(cx, |this, cx| {
                                            let _ = this
                                                .app
                                                .smart_delete_literature(&lit_id_delete, &sel_ids);
                                            this.close_menus(cx);
                                        });
                                    }
                                }),
                        );
                    }
                }
            }
            menu
        })
    }
}
