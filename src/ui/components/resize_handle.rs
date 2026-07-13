use gpui::{Div, Pixels, Styled, div, rems};

#[derive(Clone, Copy)]
pub enum Side {
    Left,
    Right,
}

#[must_use]
pub fn render_resize_handle(side: Side, offset: Pixels) -> Div {
    let style = div()
        .absolute()
        .top_0()
        .h_full()
        .w(rems(0.25))
        .mx(rems(-0.125))
        .cursor_col_resize();

    match side {
        Side::Left => style.left(offset),
        Side::Right => style.right(offset),
    }
}
