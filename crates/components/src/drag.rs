#[cfg(target_os = "windows")]
use gpui::WindowControlArea;
use gpui::prelude::*;
use gpui::{App, Window};
#[cfg(not(target_os = "windows"))]
use gpui::MouseButton;

/// 为元素添加跨平台拖拽功能
///
/// - Windows: 使用 WindowControlArea::Drag 原生处理
/// - Linux/macOS: 手动 mouse 事件处理
///
/// 调用方应先设置好元素的基本属性（id、尺寸等），再调用此函数添加拖拽行为。
/// 双击行为由调用方通过 `.on_double_click()` 自行添加。
pub fn add_drag_behavior<E: InteractiveElement>(
    element: E,
    _window: &mut Window,
    _cx: &mut App,
) -> E {
    // Windows: drag handled natively
    #[cfg(target_os = "windows")]
    let element = element.window_control_area(WindowControlArea::Drag);

    // Linux/macOS: manual mouse event handling for window drag
    #[cfg(not(target_os = "windows"))]
    let element = {
        let state = _window.use_state(_cx, |_, _| false);
        element
            .on_mouse_down(MouseButton::Left, {
                let s = state.clone();
                move |_, _, cx| {
                    s.update(cx, |val, _| *val = true);
                    cx.stop_propagation();
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let s = state.clone();
                move |_, _, cx| {
                    s.update(cx, |val, _| *val = false);
                }
            })
            .on_mouse_move(move |_, window, cx| {
                if *state.read(cx) {
                    state.update(cx, |val, _| *val = false);
                    window.start_window_move();
                }
            })
    };

    element
}
