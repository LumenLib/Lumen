use gpui::prelude::*;
use gpui::{App, MouseButton, SharedString, Window, WindowControlArea, div, rems};
use gpui_component::{ActiveTheme, Icon, Sizable, h_flex};

/// 创建跨平台窗口控件（min/max/close）
pub fn make_window_controls(window: &Window, cx: &App) -> impl IntoElement {
    let theme = cx.theme();

    h_flex()
        .id("window-controls")
        .h_full()
        .items_center()
        .gap_0()
        .child(make_control_button(
            SharedString::from("win-minimize"),
            gpui_component::IconName::WindowMinimize,
            theme.foreground,
            theme.secondary_hover,
            theme.secondary_foreground,
            WindowControlArea::Min,
            |window| window.minimize_window(),
        ))
        .child(make_control_button(
            SharedString::from("win-maximize"),
            if window.is_maximized() {
                gpui_component::IconName::WindowRestore
            } else {
                gpui_component::IconName::WindowMaximize
            },
            theme.foreground,
            theme.secondary_hover,
            theme.secondary_foreground,
            WindowControlArea::Max,
            |window| window.zoom_window(),
        ))
        .child(make_control_button(
            SharedString::from("win-close"),
            gpui_component::IconName::WindowClose,
            theme.foreground,
            theme.danger,
            theme.danger_foreground,
            WindowControlArea::Close,
            |window| window.remove_window(),
        ))
}

fn make_control_button(
    id: SharedString,
    icon: gpui_component::IconName,
    color: gpui::Hsla,
    hover_bg: gpui::Hsla,
    hover_fg: gpui::Hsla,
    control_area: WindowControlArea,
    on_click: impl Fn(&mut Window) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .w(rems(1.5))
        .h(rems(1.5))
        .flex_shrink_0()
        .justify_center()
        .items_center()
        .text_color(color)
        .hover(move |s| s.bg(hover_bg).text_color(hover_fg))
        .when(cfg!(windows), move |this| {
            this.window_control_area(control_area)
        })
        .when(cfg!(not(windows)), |this| {
            this.on_mouse_down(MouseButton::Left, |_, window, cx| {
                if cfg!(target_os = "linux") {
                    window.prevent_default();
                }
                cx.stop_propagation();
            })
            .on_click(move |_, window, _| on_click(window))
        })
        .child(Icon::new(icon).small())
}
