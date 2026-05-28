use crate::services::main_app::MainApp;
use crate::services::ui_state::UiState;
use crate::ui::icons::IconName;
use gpui::prelude::*;
use gpui::{AnyElement, MouseButton, Window, div, rems};
use gpui_component::{
    ActiveTheme, Icon, Sizable, Theme, h_flex, scroll::ScrollableElement, v_flex,
};
use i18n::{I18nKey, t};
use models::{Folder, FolderType};
use std::sync::Arc;

pub type FolderSelectCallback =
    Box<dyn Fn(Option<String>, &mut Window, &mut Context<FolderSelector>) + Send + Sync>;

/// 文件夹选择器组件（用于二级菜单）
pub struct FolderSelector {
    app: Arc<MainApp>,
    folders: Vec<Folder>,
    /// 选中的回调：Option<String> 为 None 表示选择了"所有文献"
    on_select: FolderSelectCallback,
    show_all_option: bool,
}

impl FolderSelector {
    pub fn new(
        app: Arc<MainApp>,
        folders: Vec<Folder>,
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

    /// 检查文件夹是否展开（从 `UiState` 读取）
    fn is_expanded(&self, id: &str, ui: &UiState) -> bool {
        ui.menu_folder_expanded.contains(id)
    }

    /// 切换文件夹展开状态（更新 `UiState Global`）
    fn toggle_expand(&mut self, id: String, cx: &mut Context<Self>) {
        UiState::update(cx, |state| {
            if state.menu_folder_expanded.contains(&id) {
                state.menu_folder_expanded.remove(&id);
            } else {
                state.menu_folder_expanded.insert(id);
            }
        });
    }

    fn render_folder_item(
        &self,
        folder: &Folder,
        depth: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let folder_id = folder.id.clone();
        let name = folder.name.clone();
        let has_children = self
            .folders
            .iter()
            .any(|f| f.parent_id == Some(folder_id.clone()));
        let ui = cx.global::<UiState>();
        let is_expanded = self.is_expanded(&folder_id, &ui);

        let item = div()
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
                    let folder_id = folder_id.clone();
                    move |this, _, window, cx| {
                        cx.stop_propagation();
                        (this.on_select)(Some(folder_id.clone()), window, cx);
                    }
                }),
            )
            .child(
                h_flex()
                    .gap_1()
                    .pl(rems(depth as f32 * 0.75))
                    .items_center()
                    .child(if has_children {
                        div()
                            .w(rems(0.75))
                            .h(rems(1.25))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(theme.muted_foreground)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener({
                                    let folder_id = folder_id.clone();
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.toggle_expand(folder_id.clone(), cx);
                                    }
                                }),
                            )
                            .child(
                                Icon::new(if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .xsmall(),
                            )
                    } else {
                        div().w(rems(0.75)).h(rems(1.25))
                    })
                    .child(
                        Icon::new(IconName::Folder)
                            .small()
                            .text_color(theme.foreground),
                    )
                    .child(div().text_sm().text_color(theme.foreground).child(name)),
            );

        let mut elements = vec![item.into_any_element()];

        if is_expanded {
            elements.extend(self.render_tree(Some(folder_id), depth + 1, theme, cx));
        }

        v_flex().children(elements).into_any_element()
    }

    fn render_tree(
        &self,
        parent_id: Option<String>,
        depth: usize,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut target_folders: Vec<_> = self
            .folders
            .iter()
            .filter(|f| f.folder_type == FolderType::Custom && f.parent_id == parent_id)
            .collect();
        target_folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        target_folders
            .into_iter()
            .map(|f| self.render_folder_item(f, depth, theme, cx))
            .collect()
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

        items.extend(self.render_tree(None, 0, &theme, cx));

        v_flex()
            .w_full()
            .max_h(rems(20.0))
            .overflow_y_scrollbar()
            .children(items)
    }
}
