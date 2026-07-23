use gpui::{App, Entity, InteractiveElement, IntoElement, ParentElement, Render, Styled, div};
use gpui_component::ActiveTheme;

// Optimized implementation matching the project's existing pattern simpler:
// Just a function or a simple struct that returns the styled div.

#[must_use]
pub fn render_modal_overlay<V: Render>(modal: Entity<V>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .absolute()
        .size_full()
        .occlude() // 物理拦截：确保整个遮罩层都不允许鼠标穿透
        .bg(theme.overlay)
        .flex()
        .items_center()
        .justify_center()
        .child(modal)
}
