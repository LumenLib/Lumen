use gpui::{Div, InteractiveElement, Pixels, Styled, div, rems};

#[derive(Clone, Copy)]
pub enum Side {
    Left,
    Right,
}

#[must_use]
pub fn render_resize_handle(side: Side, offset: Pixels) -> Div {
    let handle_size = rems(0.375); // 约 6px 宽的拖拽热区
    let half_offset = rems(-0.1875); // -3px 偏置，用于居中

    let handle = div()
        .absolute()
        .top_0()
        .h_full()
        .w(handle_size)
        .mx(half_offset)
        .cursor_col_resize()
        .occlude(); // 阻断点击穿透到下方的滚动条

    match side {
        Side::Left => handle.left(offset),
        Side::Right => handle.right(offset),
    }
}
