use crate::services::main_app::MainApp;
use components::IconName;
use gpui::prelude::*;
use gpui::{MouseButton, Window, div, rems};
use gpui_component::{ActiveTheme, Icon, Sizable, h_flex, scroll::ScrollableElement, v_flex};
use i18n::{I18nKey, t};
use models::{Folder, FolderType};
use std::sync::Arc;

pub type FolderSelectCallback =
    Box<dyn Fn(Option<String>, &mut Window, &mut Context<FolderSelector>) + Send + Sync>;

/// 文件夹选择器组件（用于二级菜单）
pub struct FolderSelector {
    app: Arc<MainApp>,
    folders: Vec<Arc<Folder>>,
    /// 选中的回调：Option<String> 为 None 表示选择了"所有文献"
    on_select: FolderSelectCallback,
    show_all_option: bool,
}

impl FolderSelector {
    pub fn new(
        app: Arc<MainApp>,
        folders: Vec<Arc<Folder>>,
        show_all_option: bool,
        on_select: impl Fn(Option<String>, &mut Window, &mut Context<Self>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            app,
            folders,
            on_select: Box::new(on_select),
            show_all_option,
        }
    }
}

impl Render for FolderSelector {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let lang = self.app.current_language();
        let mut items = Vec::new();

        if self.show_all_option {
            items.push(
                div()
                    .flex()
                    .w_full()
                    .py_1()
                    .px_2()
                    .rounded_sm()
                    .hover(|s| s.bg(theme.muted))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            (this.on_select)(None, window, cx);
                        }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::BookOpen)
                                    .small()
                                    .text_color(theme.foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(t(I18nKey::AllLiterature, lang)),
                            ),
                    )
                    .into_any_element(),
            );
            items.push(
                crate::ui::views::main_window::utils::render_separator(&theme).into_any_element(),
            );
        }

        // 计算自定义文件夹的扁平化长路径并进行字典排序
        let custom_folders = {
            let folders: Vec<_> = self
                .folders
                .iter()
                .filter(|f| f.folder_type == FolderType::Custom)
                .collect();
            let mut result = Vec::new();
            for folder in &folders {
                let mut path = folder.name.clone();
                let mut current = *folder;
                while let Some(ref pid) = current.parent_id {
                    if let Some(parent) = folders.iter().find(|f| &f.id == pid) {
                        path = format!("{}/{}", parent.name, path);
                        current = parent;
                    } else {
                        break;
                    }
                }
                result.push((folder.id.clone(), path));
            }
            result.sort_by_key(|a| a.1.to_lowercase());
            result
        };

        // 平铺渲染所有自定义文件夹项目
        for (folder_id, folder_path) in custom_folders {
            let folder_id_clone = folder_id.clone();
            items.push(
                div()
                    .flex()
                    .w_full()
                    .py_1()
                    .px_2()
                    .rounded_sm()
                    .hover(|s| s.bg(theme.muted))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let folder_id = folder_id_clone.clone();
                            move |this, _, window, cx| {
                                cx.stop_propagation();
                                (this.on_select)(Some(folder_id.clone()), window, cx);
                            }
                        }),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Icon::new(IconName::Folder)
                                    .small()
                                    .text_color(theme.foreground),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.foreground)
                                    .child(folder_path),
                            ),
                    )
                    .into_any_element(),
            );
        }

        v_flex()
            .w_full()
            .max_h(rems(20.0))
            .overflow_y_scrollbar()
            .children(items)
    }
}
