use std::sync::Arc;

use gpui::{App, ParentElement, SharedString, Styled, Window, rems};
use gpui_component::{
    button::Button,
    label::Label,
    menu::{DropdownMenu, PopupMenuItem},
};

/// 轻量级选项下拉组件（Button + DropdownMenu）。
///
/// `options`: `(value, label)` 列表
/// `current_value`: 当前选中的 value
/// `scrollable`: 下拉菜单是否可滚动
/// `on_change`: 选中项变化时的回调
pub fn selector(
    id: &'static str,
    options: Vec<(SharedString, SharedString)>,
    current_value: SharedString,
    scrollable: bool,
    on_change: impl Fn(SharedString, &mut Window, &mut App) + 'static,
) -> impl gpui::IntoElement {
    let current_label = options
        .iter()
        .find(|(v, _)| *v == current_value)
        .map(|(_, l)| l.clone())
        .unwrap_or_else(|| current_value.clone());

    let on_change = Arc::new(on_change);

    let width = rems(10.);
    Button::new(id)
        .w(width)
        .child(Label::new(current_label).text_sm())
        .dropdown_caret(true)
        .outline()
        .dropdown_menu_with_anchor(gpui::Anchor::TopLeft, move |menu, window, _| {
            let options = options.clone();
            let current_value = current_value.clone();
            let on_change = on_change.clone();
            let mut menu = menu;
            for (val, label) in options {
                let is_checked = val == current_value;
                let on_change = on_change.clone();
                let val_clone = val.clone();
                menu = menu.item(PopupMenuItem::new(label).checked(is_checked).on_click(
                    move |_, window, cx| {
                        on_change(val_clone.clone(), window, cx);
                    },
                ));
            }
            menu.scrollable(scrollable)
                .min_w(width.to_pixels(window.rem_size()))
        })
}
