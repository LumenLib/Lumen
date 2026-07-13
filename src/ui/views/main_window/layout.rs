use super::MainWindow;
use crate::ui::components::resize_handle::{Side, render_resize_handle};
use gpui::prelude::*;
use gpui::{MouseButton, Pixels};

pub fn render_left_resizer(width: Pixels, cx: &mut Context<MainWindow>) -> impl IntoElement {
    render_resize_handle(Side::Left, width).on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, _, _, cx| {
            this.dragging_left = true;
            cx.notify();
        }),
    )
}

pub fn render_right_resizer(width: Pixels, cx: &mut Context<MainWindow>) -> impl IntoElement {
    render_resize_handle(Side::Right, width).on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, _, _, cx| {
            this.dragging_right = true;
            cx.notify();
        }),
    )
}
