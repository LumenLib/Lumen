use super::{DraggedSidebar, MainWindow};
use components::{Side, render_resize_handle};
use gpui::Pixels;
use gpui::prelude::*;

pub fn render_left_resizer(width: Pixels, _cx: &mut Context<MainWindow>) -> impl IntoElement {
    let handle = render_resize_handle(Side::Left, width)
        .id("left-resizer")
        .on_drag(DraggedSidebar(Side::Left), |drag, _, _, cx| {
            cx.new(|_| drag.clone())
        });
    gpui::deferred(handle)
}

pub fn render_right_resizer(width: Pixels, _cx: &mut Context<MainWindow>) -> impl IntoElement {
    let handle = render_resize_handle(Side::Right, width)
        .id("right-resizer")
        .on_drag(DraggedSidebar(Side::Right), |drag, _, _, cx| {
            cx.new(|_| drag.clone())
        });
    gpui::deferred(handle)
}
