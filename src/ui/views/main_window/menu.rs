use super::{FetchSource, MainWindow, ViewEvent};

use crate::RUNTIME;
use crate::notification_bus::show_notification;
use crate::ui::{components::FolderSelector, icons::IconName};
use anyhow::{Error, anyhow};
use gpui::prelude::*;
use gpui::{
    AnyElement, AppContext, AsyncApp, Div, MouseButton, PathPromptOptions, Pixels, Point,
    SharedString, Stateful, Window, div, px, rems,
};
use gpui_component::notification::NotificationType;
use gpui_component::{ActiveTheme, Colorize, Icon, Sizable, Theme, h_flex, v_flex};
use i18n::{I18nKey, t};
use log::{error, info};
use models::FolderType;

/// 右键菜单类型
#[derive(Clone, Debug)]
pub enum ContextMenuType {
    Folder(Option<String>),       // 文件夹ID，None表示空白处
    Tag(Option<String>),          // 标签ID，None表示空白处
    Subscription(Option<String>), // 订阅ID，None表示空白处
    SubscriptionItem(String),     // 订阅条目ID
    Literature(String),           // 文献ID
    AddToFolder(String),          // 文献ID，展示文件夹树子菜单
    RestoreToFolder(String),      // 文献ID，展示还原文件夹树子菜单
    Attachment(String),           // 附件ID
    FetchFrom(String),            // 文献ID，展示解析来源子菜单
    BatchFetchFrom(Vec<String>),  // 文献ID列表，展示批量解析来源子菜单
}

impl MainWindow {
    /// 关闭所有活动的右键菜单和子菜单
    pub fn close_menus(&mut self, cx: &mut Context<Self>) {
        self.context_menu = None;
        self.active_submenu = None;
        cx.notify();
    }

    pub fn show_context_menu(
        &mut self,
        pos: Point<Pixels>,
        menu_type: ContextMenuType,
        cx: &mut Context<Self>,
    ) {
        self.context_menu = Some((pos, menu_type));
        self.active_submenu = None;
        self.folder_selector = None;
        cx.notify();
    }

    pub fn show_folder_selector(
        &mut self,
        pos: Point<Pixels>,
        menu_type: ContextMenuType,
        show_all: bool,
        cx: &mut Context<Self>,
    ) {
        let folders = self.data_store.read(cx).folders.clone();

        let lit_id = match &menu_type {
            ContextMenuType::AddToFolder(id) => id.clone(),
            ContextMenuType::RestoreToFolder(id) => id.clone(),
            _ => return,
        };

        let app = self.app.clone();

        let folder_selector = cx.new(|_| {
            FolderSelector::new(app.clone(), folders, show_all, {
                let lid = lit_id.clone();
                let menu_type = menu_type.clone();
                move |folder_id: Option<String>, _, cx: &mut Context<FolderSelector>| {
                    let sel_ids = {
                        let ui = cx.global::<crate::services::ui_state::UiState>();
                        ui.selected_literature_ids.clone()
                    };
                    let _is_batch = sel_ids.contains(&lid);

                    match menu_type {
                        ContextMenuType::AddToFolder(_) => {
                            if let Some(fid) = folder_id {
                                let _ = app.smart_add_literatures_to_folder(&lid, &fid, &sel_ids);
                            }
                        }
                        ContextMenuType::RestoreToFolder(_) => {
                            let _ =
                                app.smart_restore_literatures(&lid, folder_id.as_deref(), &sel_ids);
                        }
                        _ => {}
                    }
                    cx.emit(ViewEvent::CloseMenu);
                }
            })
        });

        cx.subscribe(
            &folder_selector,
            move |this: &mut MainWindow, _, event, cx| match event {
                ViewEvent::CloseMenu => {
                    this.close_menus(cx);
                }
            },
        )
        .detach();

        self.active_submenu = Some((pos, menu_type));
        self.folder_selector = Some(folder_selector);
        cx.notify();
    }

    pub(super) fn render_global_context_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let (pos, menu_type) = self.context_menu.as_ref()?;
        let mut pos = *pos;
        let theme = cx.theme().clone();
        let menu_width = px(160.0);

        // 边界检测：防止菜单超出右侧
        if pos.x + menu_width > self.current_window_width {
            pos.x -= menu_width;
        }

        // Y轴边界检测：防止菜单超出底部
        // 假设菜单最大高度约 300px (根据项目数量估算)

        let mut menu_div = div()
            .absolute()
            .left(pos.x)
            .w(menu_width)
            .bg(theme.popover)
            .text_color(theme.popover_foreground)
            .rounded_md()
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .flex()
            .flex_col()
            .p_1()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.active_submenu = None;
                    cx.notify();
                }),
            );

        if super::utils::is_menu_upward(pos.y, self.current_window_height) {
            // 向上弹出
            menu_div = menu_div.bottom(self.current_window_height - pos.y);
        } else {
            // 向下弹出
            menu_div = menu_div.top(pos.y);
        }

        Some(menu_div.children(match menu_type {
            ContextMenuType::Folder(target_id) => {
                self.render_folder_menu_items(target_id.clone(), &theme, cx)
            }
            ContextMenuType::Tag(target_id) => {
                self.render_tag_menu_items(target_id.clone(), &theme, cx)
            }
            ContextMenuType::Subscription(target_id) => {
                self.render_subscription_menu_items(target_id.clone(), &theme, cx)
            }
            ContextMenuType::SubscriptionItem(sub_id) => {
                self.render_subscription_item_menu_items(sub_id.clone(), &theme, cx)
            }
            ContextMenuType::Literature(lit_id) => {
                self.render_literature_menu_items(lit_id.clone(), pos, &theme, cx)
            }
            ContextMenuType::Attachment(att_id) => {
                self.render_attachment_menu_items(att_id.clone(), &theme, cx)
            }
            ContextMenuType::FetchFrom(lit_id) => {
                // 子菜单内容 en render_submenu 中渲染，这里只渲染主菜单
                self.render_literature_menu_items(lit_id.clone(), pos, &theme, cx)
            }
            ContextMenuType::BatchFetchFrom(lit_ids) => {
                self.render_literature_menu_items(lit_ids[0].clone(), pos, &theme, cx)
            }
            ContextMenuType::AddToFolder(_) | ContextMenuType::RestoreToFolder(_) => vec![],
        }))
    }

    fn render_subscription_item_menu_items(
        &self,
        sub_id: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut items = Vec::new();
        let sub_id_clone = sub_id.clone();

        // Check if we are in batch mode
        let (is_batch, selected_ids) = {
            let ui = cx.global::<crate::services::ui_state::UiState>();
            (
                ui.selected_feed_item_ids.contains(&sub_id),
                ui.selected_feed_item_ids.clone(),
            )
        };

        let (is_read, url_opt) = {
            let data = self.data_store.read(cx);
            if let Some(s) = data.feed_items.iter().find(|s| s.id == sub_id) {
                (s.is_read, s.url.clone())
            } else {
                (false, None)
            }
        };

        let lang = self.app.current_language();

        let label = if is_read {
            t(I18nKey::MarkAsUnread, lang)
        } else {
            t(I18nKey::MarkAsRead, lang)
        };
        let icon = IconName::Bell;

        let _selected_ids_mark = selected_ids.clone();

        // 0. Add to Library (only if not added and not batch ?)
        // Actually earlier implementation only allowed single item add.

        let is_added = {
            let data = self.data_store.read(cx);
            if let Some(s) = data.feed_items.iter().find(|s| s.id == sub_id) {
                s.is_added_to_library
            } else {
                false
            }
        };

        if !is_added && !is_batch {
            let sub_id_add = sub_id.clone();
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::AddToLibrary, lang),
                    move |this, _, cx| {
                        let id = sub_id_add.clone();
                        let app = this.app.clone();
                        cx.spawn(move |_, _: &mut AsyncApp| async move {
                            if let Err(e) = app.add_feed_item_to_library(&id) {
                                error!("添加到文献库失败: {e}");
                            }
                        })
                        .detach();
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        if let Some(url) = url_opt
            && !url.trim().is_empty()
        {
            let url_clone = url.clone();
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Globe)
                            .small()
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::OpenInBrowser, lang),
                    move |this, _, cx| {
                        super::utils::open_url(&url_clone);
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(icon)
                        .small()
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                label,
                move |this, _, cx| {
                    let sel_ids = {
                        let ui = cx.global::<crate::services::ui_state::UiState>();
                        ui.selected_feed_item_ids.clone()
                    };
                    let _ =
                        this.app
                            .smart_toggle_feed_items_read(&sub_id_clone, !is_read, &sel_ids);
                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        let sub_id_del = sub_id.clone();
        let delete_label = t(I18nKey::Delete, lang);
        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(IconName::Trash)
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                delete_label,
                move |this, _, cx| {
                    let sel_ids = {
                        let ui = cx.global::<crate::services::ui_state::UiState>();
                        ui.selected_feed_item_ids.clone()
                    };
                    let _ = this.app.smart_delete_feed_items(&sub_id_del, &sel_ids);
                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        items
    }

    fn render_subscription_menu_items(
        &self,
        target_id: Option<String>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let lang = self.app.current_language();
        let mut items = Vec::new();

        if let Some(id) = target_id {
            let id_clone = id.clone();
            let id_clone_update = id.clone();

            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Globe)
                            .small()
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::UpdateSubscription, lang),
                    move |this, _, cx| {
                        let id = id_clone_update.clone();
                        let app = this.app.clone();
                        let feed_manager = app.feed_service.clone();

                        cx.spawn(move |_, _cx: &mut AsyncApp| {
                            let app = app.clone();
                            async move {
                                if let Err(e) = feed_manager.refresh_feed(app, id).await {
                                    error!("更新订阅失败: {e}");
                                }
                            }
                        })
                        .detach();

                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Edit)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::EditSubscription, lang),
                    move |this, _window, cx| {
                        let id = id_clone.clone();
                        this.open_edit_subscription_modal(id, cx);
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            let id_clone_del = id.clone();
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Trash)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::Unsubscribe, lang),
                    move |this, _, cx| {
                        let _ = this.app.delete_feed(&id_clone_del);
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        } else {
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::AddSubscription, lang),
                    move |this, _window, cx| {
                        this.open_add_subscription_modal(cx);
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        items
    }

    fn render_folder_menu_items(
        &self,
        target_id: Option<String>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let lang = self.app.current_language();
        let mut items = Vec::new();

        if let Some(id) = target_id {
            if id == "trash" {
                items.push(
                    self.render_menu_item(
                        Some(
                            Icon::new(IconName::Trash)
                                .text_color(theme.foreground)
                                .into_any_element(),
                        ),
                        t(I18nKey::EmptyTrash, lang),
                        move |this, _window, cx| {
                            info!("UI: User clicked Empty Trash");
                            this.close_menus(cx);
                            this.handle_empty_trash(cx);
                        },
                        theme,
                        cx,
                    )
                    .into_any_element(),
                );
                return items;
            }

            let id_clone = id.clone();
            let id_clone2 = id.clone();
            let id_clone3 = id.clone();

            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::NewSubFolder, lang),
                    move |this, window, cx| {
                        let id = id_clone.clone();
                        this.literature_panel
                            .update(cx, |p, cx| p.add_folder(Some(id), window, cx));
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Edit)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::Rename, lang),
                    move |this, window, cx| {
                        let id = id_clone2.clone();
                        this.literature_panel
                            .update(cx, |p, cx| p.start_rename(id, false, window, cx));
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Trash)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::Delete, lang),
                    move |this, _, cx| {
                        let id = id_clone3.clone();
                        this.literature_panel
                            .update(cx, |p, cx| p.delete_folder(id, cx));
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        } else {
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::NewFolder, lang),
                    move |this, window, cx| {
                        this.literature_panel
                            .update(cx, |p, cx| p.add_folder(None, window, cx));
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        items
    }

    fn render_tag_menu_items(
        &self,
        target_id: Option<String>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let lang = self.app.current_language();
        let mut items = Vec::new();

        if let Some(tid) = target_id {
            let tid_clone = tid.clone();
            let tid_clone_del = tid.clone();

            // 重命名
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Edit)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::Rename, lang),
                    move |this, window, cx| {
                        let tid = tid_clone.clone();
                        this.literature_panel
                            .update(cx, |p, cx| p.start_tag_rename(tid, false, window, cx));
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            // 颜色选择
            items.push(super::utils::render_separator(theme).into_any_element());
            items.push(
                self.render_tag_color_picker(&tid, theme, cx)
                    .into_any_element(),
            );
            items.push(super::utils::render_separator(theme).into_any_element());

            // 删除
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Trash)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::Delete, lang),
                    move |this, _, cx| {
                        let tid = tid_clone_del.clone();
                        this.literature_panel
                            .update(cx, |p, cx| p.delete_tag(tid, cx));
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }
        // 暂不实现空白区"新建标签"菜单

        items
    }

    fn render_tag_color_picker(
        &self,
        tid: &str,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // 获取当前标签颜色和名称
        let (tag_name, current_color) = {
            let data = self.data_store.read(cx);
            data.tags
                .iter()
                .find(|(t, _)| t.id == tid)
                .map(|(t, _)| (t.name.clone(), t.color.clone()))
                .unwrap_or_default()
        };

        let tag_colors_ref = models::tag::TAG_COLORS;
        let tag_colors: Vec<&str> = tag_colors_ref.iter().map(|(_, hex)| *hex).collect();

        let tid_inner = tid.to_string();
        let theme_inner = theme.clone();

        let render_color_row = |colors: &[&str],
                                current_color: String,
                                tag_name: String,
                                theme: Theme,
                                tid: String,
                                cx: &mut Context<Self>| {
            h_flex()
                .w_full()
                .justify_around()
                .gap_1()
                .py_1()
                .children(colors.iter().map(move |&color_hex| {
                    let color_hex = color_hex.to_string();
                    let is_active = current_color == color_hex;
                    let color_hex_clone = color_hex.clone();
                    let tid_clone = tid.clone();
                    let tag_name_clone = tag_name.clone();
                    let theme_clone = theme.clone();

                    let color = gpui::Hsla::parse_hex(&color_hex).unwrap_or(gpui::red());

                    div()
                        .id(SharedString::from(format!("color-{color_hex}")))
                        .size(rems(0.75))
                        .rounded_full()
                        .bg(color)
                        .cursor_pointer()
                        .border_2()
                        .border_color(if is_active {
                            theme_clone.foreground
                        } else {
                            gpui::Hsla::transparent_black()
                        })
                        .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                            let _ = this.app.tag_service.update_tag(
                                &this.app,
                                &tid_clone,
                                &tag_name_clone,
                                &color_hex_clone,
                            );
                            this.app.notify_data_changed();
                            this.close_menus(cx);
                        }))
                        .child(if is_active {
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Check)
                                        .size(rems(0.5))
                                        .text_color(gpui::white()),
                                )
                        } else {
                            div()
                        })
                }))
        };

        v_flex()
            .px_2()
            .py_1()
            .gap_1()
            .children(tag_colors.chunks(5).map(|chunk| {
                render_color_row(
                    chunk,
                    current_color.clone(),
                    tag_name.clone(),
                    theme_inner.clone(),
                    tid_inner.clone(),
                    cx,
                )
            }))
    }

    fn render_literature_menu_items(
        &self,
        lit_id: String,
        menu_pos: Point<Pixels>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let lang = self.app.current_language();
        let mut items = Vec::new();
        let id_clone = lit_id.clone();
        let id_clone_edit = lit_id.clone();

        // Check if the clicked item is part of the selection
        let (_is_batch, selected_count, selected_ids) = {
            let ui = cx.global::<crate::services::ui_state::UiState>();
            let ids: Vec<String> = ui.selected_literature_ids.iter().cloned().collect();
            (
                ui.selected_literature_ids.contains(&lit_id),
                ui.selected_literature_ids.len(),
                ids,
            )
        };

        // 1. 编辑 (多选时不显示)
        if selected_count <= 1 {
            items.push(
                self.render_menu_item(
                    Some(
                        Icon::new(IconName::Edit)
                            .text_color(theme.foreground)
                            .into_any_element(),
                    ),
                    t(I18nKey::Edit, lang),
                    move |this, _window, cx| {
                        this.open_edit_modal(Some(id_clone_edit.clone()), cx);
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        } else {
            // 多选时的批量操作
            let selected_ids_fetch = selected_ids.clone();
            items.push(
                self.render_menu_item_with_submenu(
                    IconName::Cloud,
                    t(I18nKey::BatchFetchMetadata, lang),
                    menu_pos,
                    move |this, pos, is_upward, _| {
                        let y_offset = super::utils::calculate_fetch_submenu_y_offset(is_upward);
                        let sub_pos = Point::new(pos.x + px(160.0), pos.y + y_offset);
                        this.active_submenu = Some((
                            sub_pos,
                            ContextMenuType::BatchFetchFrom(selected_ids_fetch.clone()),
                        ));
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }
        // 检查文献是否在回收站中
        let in_trash = {
            let data = self.data_store.read(cx);
            data.literatures
                .iter()
                .find(|lit| lit.id == lit_id)
                .is_some_and(|lit| lit.folder_ids.contains(&"trash".to_string()))
        };

        // 2. 删除 (根据是否在回收站显示不同的文字)
        let delete_label = if in_trash {
            t(I18nKey::PermanentDelete, lang)
        } else {
            t(I18nKey::Delete, lang)
        };

        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(IconName::Trash)
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                delete_label,
                move |this, _, cx| {
                    let sel_ids = {
                        let ui = cx.global::<crate::services::ui_state::UiState>();
                        ui.selected_literature_ids.clone()
                    };
                    let _ = this.app.smart_delete_literature(&id_clone, &sel_ids);
                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        items.push(super::utils::render_separator(theme).into_any_element());

        // 复制引用
        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(IconName::Copy)
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                t(I18nKey::CopyCitation, lang),
                move |this, _window, cx| {
                    this.open_citation_popup(cx);
                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        // 从...获取菜单 (多选时不显示)
        if selected_count <= 1 {
            let lit_id_fetch = lit_id.clone();
            items.push(
                self.render_menu_item_with_submenu(
                    IconName::Cloud,
                    t(I18nKey::FetchFrom, lang),
                    menu_pos,
                    move |this, pos, is_upward, _| {
                        let y_offset = super::utils::calculate_fetch_submenu_y_offset(is_upward);
                        let sub_pos = Point::new(pos.x + px(160.0), pos.y + y_offset);
                        this.active_submenu =
                            Some((sub_pos, ContextMenuType::FetchFrom(lit_id_fetch.clone())));
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        // 只有不在回收站中的文献才显示"添加到"和"还原到"菜单
        if in_trash {
            // 在回收站中，显示"还原到"按钮
            let id_clone_restore = lit_id.clone();
            items.push(
                self.render_menu_item_with_submenu(
                    IconName::Undo,
                    t(I18nKey::RestoreTo, lang),
                    menu_pos,
                    move |this, pos, is_upward, cx| {
                        let y_offset = super::utils::calculate_folder_submenu_y_offset(
                            selected_count,
                            is_upward,
                        );
                        let sub_pos = Point::new(pos.x + px(160.0), pos.y + y_offset);
                        this.show_folder_selector(
                            sub_pos,
                            ContextMenuType::RestoreToFolder(id_clone_restore.clone()),
                            true,
                            cx,
                        );
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        } else {
            // 4. 添加到
            let has_custom_folders = {
                let data = self.data_store.read(cx);
                data.folders
                    .iter()
                    .any(|f| f.folder_type == FolderType::Custom)
            };

            if has_custom_folders {
                let id_clone2 = lit_id.clone();
                items.push(
                    self.render_menu_item_with_submenu(
                        IconName::Folder,
                        t(I18nKey::AddTo, lang),
                        menu_pos,
                        move |this, pos, is_upward, cx| {
                            let y_offset = super::utils::calculate_folder_submenu_y_offset(
                                selected_count,
                                is_upward,
                            );
                            let sub_pos = Point::new(pos.x + px(160.0), pos.y + y_offset);
                            this.show_folder_selector(
                                sub_pos,
                                ContextMenuType::AddToFolder(id_clone2.clone()),
                                false,
                                cx,
                            );
                        },
                        theme,
                        cx,
                    )
                    .into_any_element(),
                );
            }

            // 只有当当前选中的是用户自建文件夹时，才显示"从文件夹移除"
            let current_selected_folder = cx
                .global::<crate::services::ui_state::UiState>()
                .selected_folder_id
                .clone();

            if let Some(folder_id) = current_selected_folder
                && folder_id != "all"
                && folder_id != "uncategorized"
                && folder_id != "trash"
            {
                let id_clone3 = lit_id.clone();
                let folder_id_clone = folder_id.clone();
                items.push(super::utils::render_separator(theme).into_any_element());
                items.push(
                    self.render_menu_item(
                        Some(
                            Icon::new(IconName::FolderOpen)
                                .small()
                                .text_color(theme.foreground)
                                .into_any_element(),
                        ),
                        t(I18nKey::RemoveFromFolder, lang),
                        move |this, _, cx| {
                            let sel_ids = {
                                let ui = cx.global::<crate::services::ui_state::UiState>();
                                ui.selected_literature_ids.clone()
                            };
                            let _ = this.app.smart_remove_literatures_from_folder(
                                &id_clone3,
                                &folder_id_clone,
                                &sel_ids,
                            );
                            this.close_menus(cx);
                        },
                        theme,
                        cx,
                    )
                    .into_any_element(),
                );
            }
        }

        items
    }

    fn render_fetch_menu_items(
        &self,
        lit_id: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut items = Vec::new();
        let lang = self.app.current_language();
        let lit = {
            let data = self.data_store.read(cx);
            data.literatures.iter().find(|l| l.id == lit_id).cloned()
        };

        if let Some(lit) = lit {
            let fetch_failed = t(I18nKey::FetchFailed, lang);
            // 1. ArXiv
            let lit_clone = lit.clone();
            let err_arxiv = t(I18nKey::FetchFailedArxiv, lang);
            items.push(
                self.render_menu_item(
                    None,
                    "ArXiv",
                    move |this, window, cx| {
                        let arxiv_id = super::utils::extract_arxiv_id(&lit_clone);
                        if let Some(id) = arxiv_id {
                            this.start_fetch_and_compare(
                                lit_clone.clone(),
                                FetchSource::ArXiv(id),
                                window,
                                cx,
                            );
                        } else {
                            show_notification(
                                NotificationType::Error,
                                format!("{}: {}", fetch_failed, err_arxiv),
                                cx,
                            );
                        }
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            // 2. DBLP (Title only)
            let lit_clone = lit.clone();
            let err_dblp = t(I18nKey::FetchFailedDblp, lang);
            items.push(
                self.render_menu_item(
                    None,
                    "DBLP",
                    move |this, window, cx| {
                        // DBLP 仅使用标题搜索
                        if lit_clone.title.is_empty() {
                            show_notification(
                                NotificationType::Error,
                                format!("{}: {}", fetch_failed, err_dblp),
                                cx,
                            );
                        } else {
                            this.start_fetch_and_compare(
                                lit_clone.clone(),
                                FetchSource::Dblp(lit_clone.title.clone()),
                                window,
                                cx,
                            );
                        }
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            // 3. DOI (Crossref)
            let lit_clone = lit.clone();
            let err_crossref = t(I18nKey::FetchFailedCrossref, lang);
            items.push(
                self.render_menu_item(
                    None,
                    "DOI (Crossref)",
                    move |this, window, cx| {
                        if let Some(ref doi) = lit_clone.doi
                            && !doi.trim().is_empty()
                        {
                            this.start_fetch_and_compare(
                                lit_clone.clone(),
                                FetchSource::Doi(doi.clone()),
                                window,
                                cx,
                            );
                        } else {
                            show_notification(
                                NotificationType::Error,
                                format!("{}: {}", fetch_failed, err_crossref),
                                cx,
                            );
                        }
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );

            // 4. OpenAlex (DOI -> Title)
            let lit_clone = lit.clone();
            let err_openalex = t(I18nKey::FetchFailedOpenAlex, lang);
            items.push(
                self.render_menu_item(
                    None,
                    "OpenAlex",
                    move |this, window, cx| {
                        let source = if let Some(ref doi) = lit_clone.doi
                            && !doi.trim().is_empty()
                        {
                            Some(FetchSource::OpenAlexDoi(doi.clone()))
                        } else if !lit_clone.title.is_empty() {
                            Some(FetchSource::OpenAlexTitle(lit_clone.title.clone()))
                        } else {
                            None
                        };

                        if let Some(s) = source {
                            this.start_fetch_and_compare(lit_clone.clone(), s, window, cx);
                        } else {
                            show_notification(
                                NotificationType::Error,
                                format!("{}: {}", fetch_failed, err_openalex),
                                cx,
                            );
                        }
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        items
    }

    fn render_batch_fetch_menu_items(
        &self,
        lit_ids: Vec<String>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut items = Vec::new();
        let sources = [
            (
                "ArXiv",
                crate::ui::views::main_window::types::BatchSource::ArXiv,
            ),
            (
                "DBLP",
                crate::ui::views::main_window::types::BatchSource::Dblp,
            ),
            (
                "DOI (Crossref)",
                crate::ui::views::main_window::types::BatchSource::Doi,
            ),
            (
                "OpenAlex",
                crate::ui::views::main_window::types::BatchSource::OpenAlex,
            ),
        ];

        for (name, source) in sources {
            let lit_ids_clone = lit_ids.clone();
            items.push(
                self.render_menu_item(
                    None,
                    name,
                    move |this, _, cx| {
                        this.handle_batch_fetch_metadata(lit_ids_clone.clone(), source, cx);
                        this.close_menus(cx);
                    },
                    theme,
                    cx,
                )
                .into_any_element(),
            );
        }

        items
    }

    pub(super) fn render_attachment_menu_items(
        &self,
        att_id: String,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let lang = self.app.current_language();
        let mut items = Vec::new();
        let att_id_clone = att_id.clone();
        let att_id_del = att_id.clone();
        let att_id_reveal = att_id.clone();

        // 查找附件信息
        let (_attachment, _lit_id) = {
            let data = self.data_store.read(cx);
            let mut result = None;
            for lit in &data.literatures {
                if let Some(att) = lit.attachments.iter().find(|a| a.id == att_id) {
                    result = Some((att.clone(), lit.id.clone()));
                    break;
                }
            }
            match result {
                Some((a, l)) => (Some(a), Some(l)),
                None => (None, None),
            }
        };

        // 显示文件夹功能
        let reveal_label = if cfg!(target_os = "macos") {
            t(I18nKey::RevealInFinder, lang)
        } else {
            t(I18nKey::RevealInExplorer, lang)
        };

        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(IconName::Folder)
                        .small()
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                reveal_label,
                move |this, _window, cx| {
                    if let Err(e) = this.app.reveal_in_explorer(&att_id_reveal) {
                        error!("在资源管理器中显示失败: {e}");
                    }
                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        items.push(super::utils::render_separator(theme).into_any_element());

        // 更换功能
        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(IconName::Undo)
                        .small()
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                t(I18nKey::ReplaceFile, lang),
                move |this, _window, cx| {
                    let att_id = att_id_clone.clone();
                    let app = this.app.clone();
                    let lang = app.current_language();

                    // 在 GPUI 线程中从 DataStore 读取，再传给 tokio 后台任务
                    let cached_lit_data = this
                        .data_store
                        .read(cx)
                        .literatures
                        .iter()
                        .find(|l| l.attachments.iter().any(|a| a.id == att_id))
                        .map(|l| {
                            let am = l.attachments.iter().find(|a| a.id == att_id).unwrap();
                            (l.id.clone(), am.is_main)
                        });

                    // 1. 发起异步文件选择请求
                    let receiver = cx.prompt_for_paths(PathPromptOptions {
                        files: true,
                        directories: false,
                        multiple: false,
                        prompt: Some(t(I18nKey::SelectNewFile, lang).into()),
                    });

                    // 2. 使用全局 Runtime 开启后台任务
                    RUNTIME.spawn(async move {
                        // 等待用户操作完成 (解开两层 Result 和一层 Option)
                        if let Ok(Ok(Some(paths))) = receiver.await
                            && let Some(path) = paths.first().cloned()
                        {
                            // 3. 执行业务逻辑
                            let result = (|| {
                                let (lit_id, is_main) = cached_lit_data
                                    .ok_or_else(|| anyhow!("Literature not found"))?;

                                if is_main {
                                    app.import_file_to_literature(&lit_id, &path, true)?;
                                } else {
                                    app.delete_attachment_file(&att_id)?;
                                    app.import_file_to_literature(&lit_id, &path, false)?;
                                }
                                Ok::<(), Error>(())
                            })();

                            if let Err(e) = result {
                                error!("更换文件失败: {e}");
                            }
                        }
                    });

                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        // 删除功能
        items.push(
            self.render_menu_item(
                Some(
                    Icon::new(IconName::Trash)
                        .text_color(theme.foreground)
                        .into_any_element(),
                ),
                t(I18nKey::DeleteFile, lang),
                move |this, _, cx| {
                    let _ = this.app.delete_attachment_file(&att_id_del);
                    this.close_menus(cx);
                },
                theme,
                cx,
            )
            .into_any_element(),
        );

        items
    }

    pub(super) fn render_submenu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (pos, menu_type) = self.active_submenu.as_ref()?;
        let mut pos = *pos;
        let theme = cx.theme().clone();

        // 边界检测：如果子菜单超出窗口右侧，则向左移动
        let submenu_min_width = px(160.0);
        if pos.x + submenu_min_width > self.current_window_width {
            pos.x -= px(160.0) + submenu_min_width;
        }

        // Y 轴边界检测
        let _submenu_height_threshold = rems(20.0); // 与 FolderSelector 保持一致

        let content = match menu_type {
            ContextMenuType::AddToFolder(_) | ContextMenuType::RestoreToFolder(_) => self
                .folder_selector
                .as_ref()
                .map(|f| f.clone().into_any_element()),
            ContextMenuType::FetchFrom(lit_id) => Some(
                div()
                    .flex()
                    .flex_col()
                    .children(self.render_fetch_menu_items(lit_id.clone(), &theme, cx))
                    .into_any_element(),
            ),
            ContextMenuType::BatchFetchFrom(lit_ids) => Some(
                div()
                    .flex()
                    .flex_col()
                    .children(self.render_batch_fetch_menu_items(lit_ids.clone(), &theme, cx))
                    .into_any_element(),
            ),
            _ => None,
        };

        content.map(|element| {
            let mut menu_div = div()
                .absolute()
                .left(pos.x)
                .min_w(submenu_min_width)
                .bg(theme.popover)
                .text_color(theme.popover_foreground)
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(element);

            if super::utils::is_menu_upward(pos.y, self.current_window_height) {
                menu_div = menu_div.bottom(self.current_window_height - pos.y);
            } else {
                menu_div = menu_div.top(pos.y);
            }
            menu_div
        })
    }

    fn render_menu_item(
        &self,
        icon: Option<AnyElement>,
        label: impl Into<SharedString>,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = label.into();
        let mut container = h_flex().gap_2();

        if let Some(icon) = icon {
            container = container.child(
                div()
                    .size(rems(0.875))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon),
            );
        }

        div()
            .id(SharedString::from(format!("menu-item-{label}")))
            .flex()
            .w_full()
            .py_1()
            .px_2()
            .rounded_sm()
            .hover(|s| s.bg(theme.muted))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    on_click(this, window, cx);
                }),
            )
            .child(container.child(div().text_sm().text_color(theme.foreground).child(label)))
    }

    /// 渲染带有子菜单指示器的菜单项
    fn render_menu_item_with_submenu(
        &self,
        icon: IconName,
        label: impl Into<SharedString>,
        menu_pos: Point<Pixels>,
        on_click: impl Fn(&mut Self, Point<Pixels>, bool, &mut Context<Self>) + 'static,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let label = label.into();
        div()
            .id(SharedString::from(format!("submenu-trigger-{label}")))
            .flex()
            .w_full()
            .py_1()
            .px_2()
            .rounded_sm()
            .hover(|s| s.bg(theme.muted))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    let is_upward =
                        super::utils::is_menu_upward(menu_pos.y, this.current_window_height);
                    on_click(this, menu_pos, is_upward, cx);
                    cx.notify();
                }),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                div()
                                    .size(rems(0.875))
                                    .flex_none()
                                    .child(Icon::new(icon).text_color(theme.foreground)),
                            )
                            .child(div().text_sm().text_color(theme.foreground).child(label)),
                    )
                    .child(
                        div().size(rems(0.875)).flex_none().child(
                            Icon::new(IconName::ChevronRight)
                                .xsmall()
                                .text_color(theme.muted_foreground),
                        ),
                    ),
            )
    }
}
