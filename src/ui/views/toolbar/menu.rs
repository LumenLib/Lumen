use crate::ui::{
    components::FetchMode,
    icons::IconName,
    views::toolbar::{ToolbarEvent, ToolbarView},
};
use gpui::prelude::*;
use gpui::{WeakEntity, Window, rems};
use gpui_component::{
    Icon, h_flex,
    menu::{PopupMenu, PopupMenuItem},
};
use i18n::{I18nKey, Language, t};
use models::{Folder, FolderType};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolbarMenuTarget {
    Add,
    AddToFolder,
    None,
}

pub struct ToolbarMenuBuilder {
    pub view: WeakEntity<ToolbarView>,
}

impl ToolbarMenuBuilder {
    #[must_use]
    pub fn new(view: WeakEntity<ToolbarView>) -> Self {
        Self { view }
    }

    pub fn build(
        &self,
        target: ToolbarMenuTarget,
        lang: Language,
    ) -> impl Fn(PopupMenu, &mut Window, &mut gpui::Context<PopupMenu>) -> PopupMenu {
        let view_weak = self.view.clone();

        move |menu, _window, cx| match target {
            ToolbarMenuTarget::Add => menu
                .item(
                    PopupMenuItem::element(move |_window, _cx| {
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Edit).size(rems(1.125)))
                            .child(t(I18nKey::ManualAdd, lang))
                    })
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |_, cx| {
                                    cx.emit(ToolbarEvent::OpenManualAdd);
                                });
                            }
                        }
                    }),
                )
                .separator()
                .item(
                    PopupMenuItem::element(move |_window, _cx| {
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::File).size(rems(1.125)))
                            .child(t(I18nKey::BibTeXImport, lang))
                    })
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |_, cx| {
                                    cx.emit(ToolbarEvent::OpenFetch(FetchMode::BibTeX));
                                });
                            }
                        }
                    }),
                )
                .item(
                    PopupMenuItem::element(move |_window, _cx| {
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Plus).size(rems(1.125)))
                            .child(t(I18nKey::DoiImport, lang))
                    })
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |_, cx| {
                                    cx.emit(ToolbarEvent::OpenFetch(FetchMode::Doi));
                                });
                            }
                        }
                    }),
                )
                .item(
                    PopupMenuItem::element(move |_window, _cx| {
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Globe).size(rems(1.125)))
                            .child(t(I18nKey::ArXivImport, lang))
                    })
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |_, cx| {
                                    cx.emit(ToolbarEvent::OpenFetch(FetchMode::ArXiv));
                                });
                            }
                        }
                    }),
                )
                .item(
                    PopupMenuItem::element(move |_window, _cx| {
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::Plus).size(rems(1.125)))
                            .child(t(I18nKey::DblpSearch, lang))
                    })
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |_, cx| {
                                    cx.emit(ToolbarEvent::OpenFetch(FetchMode::Dblp));
                                });
                            }
                        }
                    }),
                ),
            ToolbarMenuTarget::AddToFolder => {
                // 获取文件夹列表
                let folders = if let Some(view) = view_weak.upgrade() {
                    view.read(cx)
                        .data_store
                        .read(cx)
                        .folders
                        .clone()
                } else {
                    vec![]
                };

                let mut result_menu = menu;

                // 添加"添加到文献库"选项（不指定文件夹）
                result_menu = result_menu.item(
                    PopupMenuItem::element(move |_window, _cx| {
                        h_flex()
                            .gap_2()
                            .child(Icon::new(IconName::BookOpen).size(rems(1.125)))
                            .child(t(I18nKey::AllLiterature, lang))
                    })
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |_, cx| {
                                    cx.emit(ToolbarEvent::AddSubscriptionToFolder(None));
                                });
                            }
                        }
                    }),
                );

                // 添加分隔符
                result_menu = result_menu.separator();

                // 递归渲染文件夹树
                fn render_folder_tree(
                    menu: PopupMenu,
                    folders: &[Folder],
                    parent_id: Option<String>,
                    depth: usize,
                    view_weak: &WeakEntity<ToolbarView>,
                ) -> PopupMenu {
                    let mut result = menu;

                    for folder in folders {
                        if folder.folder_type == FolderType::Custom && folder.parent_id == parent_id
                        {
                            let folder_id = folder.id.clone();
                            let folder_name = folder.name.clone();
                            let indent = "  ".repeat(depth);

                            // 检查是否有子文件夹
                            let has_children = folders.iter().any(|f| {
                                f.folder_type == FolderType::Custom
                                    && f.parent_id == Some(folder_id.clone())
                            });

                            let display_name = if has_children {
                                format!("{indent}{folder_name} ▸")
                            } else {
                                format!("{indent}{folder_name}")
                            };

                            result = result.item(
                                PopupMenuItem::element(move |_window, _cx| {
                                    h_flex()
                                        .gap_2()
                                        .child(Icon::new(IconName::Folder).size(rems(1.125)))
                                        .child(display_name.clone())
                                })
                                .on_click({
                                    let view_weak = view_weak.clone();
                                    let fid = folder_id.clone();
                                    move |_, _, cx| {
                                        if let Some(view) = view_weak.upgrade() {
                                            view.update(cx, |_, cx| {
                                                cx.emit(ToolbarEvent::AddSubscriptionToFolder(
                                                    Some(fid.clone()),
                                                ));
                                            });
                                        }
                                    }
                                }),
                            );

                            // 递归渲染子文件夹
                            if has_children {
                                result = render_folder_tree(
                                    result,
                                    folders,
                                    Some(folder_id),
                                    depth + 1,
                                    view_weak,
                                );
                            }
                        }
                    }

                    result
                }

                result_menu = render_folder_tree(result_menu, &folders, None, 0, &view_weak);
                result_menu
            }
            ToolbarMenuTarget::None => menu,
        }
    }
}
