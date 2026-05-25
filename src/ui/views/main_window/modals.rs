use super::MainWindow;
use crate::ui::icons::IconName;
use gpui::prelude::*;
use gpui::{
    Context, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    Point, StatefulInteractiveElement, Styled, Window, div, px, rems, rgba,
};
use gpui_component::{ActiveTheme, Icon, Sizable, h_flex, scroll::ScrollableElement, v_flex};
use std::sync::Arc;

/// 渲染错误模态框
pub fn render_error_modal(
    title: String,
    content: String,
    cx: &mut Context<MainWindow>,
) -> impl IntoElement {
    let theme = cx.theme().clone();

    div()
        .absolute()
        .bottom(rems(3.0))
        .left(rems(0.75))
        .w(rems(20.0))
        .child(
            v_flex()
                .occlude()
                .bg(theme.popover)
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .p_3()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(Icon::new(IconName::CircleX).small().text_color(gpui::red()))
                                .child(div().text_sm().font_weight(FontWeight::BOLD).child(title)),
                        )
                        .child(
                            div()
                                .id("close-global-error")
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.error_modal = None;
                                    cx.notify();
                                }))
                                .child(
                                    Icon::new(IconName::Close)
                                        .small()
                                        .text_color(theme.muted_foreground),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.foreground)
                        .whitespace_normal()
                        .max_h(rems(12.5))
                        .overflow_y_scrollbar()
                        .child(content),
                ),
        )
}

/// 通用选择器浮层
fn render_overlay_selector(
    selector: gpui::AnyElement,
    position: Point<Pixels>,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
    on_outside_click: Arc<dyn Fn(&mut MainWindow, &mut Context<MainWindow>)>,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let width = px(160.0);
    let window_size = window.bounds().size;
    let x = position.x;
    let y = position.y;
    let use_bottom = y > window_size.height / 2.0;
    let use_right_anchor = x + width > window_size.width;

    let oc_left = on_outside_click.clone();
    let oc_right = on_outside_click;

    div()
        .absolute()
        .size_full()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| (oc_left)(this, cx)),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, _, _, cx| (oc_right)(this, cx)),
        )
        .child(
            div()
                .absolute()
                .when(use_bottom, |this| this.bottom(window_size.height - y))
                .when(!use_bottom, |this| this.top(y))
                .when(use_right_anchor, |this| this.right(window_size.width - x))
                .when(!use_right_anchor, |this| this.left(x))
                .w(width)
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .shadow_xl()
                .rounded_md()
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                .child(selector),
        )
}

/// 渲染标签选择器浮层
pub fn render_tag_selector(
    this: &MainWindow,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) -> Option<impl IntoElement> {
    let (selector, position) = this.tag_selector.as_ref()?.clone();
    Some(render_overlay_selector(
        selector.into_any_element(),
        position,
        window,
        cx,
        Arc::new(|this, cx| {
            this.tag_selector = None;
            cx.notify();
        }),
    ))
}

/// 渲染文件夹选择器浮层
pub fn render_folder_selector(
    this: &MainWindow,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) -> Option<impl IntoElement> {
    let (selector, position) = this.toolbar_view.read(cx).folder_selector.as_ref()?.clone();
    Some(render_overlay_selector(
        selector.into_any_element(),
        position,
        window,
        cx,
        Arc::new(|this, cx| {
            this.toolbar_view.update(cx, |toolbar, cx| {
                toolbar.folder_selector = None;
                cx.notify();
            });
        }),
    ))
}

/// 渲染加载模态框
pub fn render_loading_modal(message: String, cx: &mut Context<MainWindow>) -> impl IntoElement {
    let theme = cx.theme().clone();
    div()
        .absolute()
        .size_full()
        .occlude()
        .bg(rgba(0x000000aa))
        .flex()
        .items_center()
        .justify_center()
        .child(
            h_flex()
                .occlude()
                .bg(theme.background)
                .p_6()
                .rounded_xl()
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .gap_4()
                .child(div().text_sm().child(message)),
        )
}
