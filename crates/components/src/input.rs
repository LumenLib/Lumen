use gpui::{App, Entity, ParentElement, SharedString, Styled, div};
use gpui_component::{
    ActiveTheme, Theme,
    input::{Input, InputState},
    v_flex,
};

/// 静音输入框：灰色背景 + 圆角 + 边框，关闭 Input 自带样式。
pub fn muted_input(input: Input, theme: &Theme) -> gpui::Div {
    div()
        .bg(theme.muted)
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .child(input.appearance(false))
}

/// 带标签的输入框：标签在上，静音输入框在下。
pub fn labeled_input(
    label: impl Into<SharedString>,
    input: &Entity<InputState>,
    cx: &mut App,
) -> gpui::Div {
    let theme = cx.theme();
    v_flex()
        .gap_1()
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(label.into()),
        )
        .child(muted_input(Input::new(input), theme))
}
