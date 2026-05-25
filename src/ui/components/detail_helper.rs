use crate::ui::icons::IconName;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, ClickEvent, CursorStyle, MouseButton, MouseDownEvent, SharedString, Window,
    div, rems,
};
use gpui_component::{Icon, Theme, h_flex, label::Label, v_flex};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type ToggleHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;

/// 渲染小图标按钮的通用函数 (类似于复制按钮)
pub fn render_icon_button(
    id: impl Into<SharedString>,
    icon: IconName,
    color: gpui::Hsla,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id.into())
        .cursor(CursorStyle::PointingHand)
        .opacity(0.0)
        .hover(|s| s.bg(theme.muted).rounded_sm())
        .group_hover("row_group", |s| s.opacity(1.0))
        .p_px()
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_click(event, window, cx);
        })
        .child(Icon::new(icon).size(rems(0.625)).text_color(color))
}

/// 渲染复制按钮的辅助函数
pub fn render_copy_button(
    id: impl Into<SharedString>,
    is_copied: bool,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let icon = if is_copied {
        IconName::Check
    } else {
        IconName::Copy
    };
    let color = if is_copied {
        theme.primary
    } else {
        theme.muted_foreground
    };

    if is_copied {
        // 如果已复制，保持显示
        div()
            .id(id.into())
            .p_px()
            .child(Icon::new(icon).size(rems(0.625)).text_color(color))
            .into_any_element()
    } else {
        render_icon_button(id, icon, color, theme, on_click).into_any_element()
    }
}

pub struct DetailRow {
    label: SharedString,
    value: SharedString,
    is_copied: bool,
    on_copy: ClickHandler,
    on_click: Option<ClickHandler>,
    child: Option<AnyElement>,
}

impl DetailRow {
    pub fn new(
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        is_copied: bool,
        on_copy: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            is_copied,
            on_copy: Box::new(on_copy),
            on_click: None,
            child: None,
        }
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.child = Some(child.into_any_element());
        self
    }

    #[must_use]
    pub fn render(mut self, theme: &Theme) -> impl IntoElement {
        let label = self.label.clone();
        let value = self.value.clone();
        let is_copied = self.is_copied;
        let on_copy = std::mem::replace(&mut self.on_copy, Box::new(|_, _, _| {}));
        let on_click = self.on_click.take();
        let extra_child = self.child.take();

        let row_id = SharedString::from(format!("row-{label}"));
        let copy_id = SharedString::from(format!("copy-{label}"));

        v_flex()
            .group("row_group")
            .gap_1()
            .mb_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(label),
                    )
                    .child(render_copy_button(copy_id, is_copied, theme, on_copy)),
            )
            .child(
                div()
                    .id(row_id)
                    .overflow_hidden()
                    .text_xs()
                    .text_color(theme.foreground)
                    .when_some(on_click, |el, handler| {
                        el.on_click(move |ev, window, cx| handler(ev, window, cx))
                    })
                    .child(value),
            )
            .when_some(extra_child, gpui::ParentElement::child)
    }
}

pub struct LinkRow {
    label: SharedString,
    text: SharedString,
    is_copied: bool,
    on_copy: ClickHandler,
    on_click: ClickHandler,
}

impl LinkRow {
    pub fn new(
        label: impl Into<SharedString>,
        text: impl Into<SharedString>,
        is_copied: bool,
        on_copy: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            is_copied,
            on_copy: Box::new(on_copy),
            on_click: Box::new(on_click),
        }
    }

    #[must_use]
    pub fn render(mut self, theme: &Theme) -> impl IntoElement {
        let label = self.label.clone();
        let text = self.text.clone();
        let is_copied = self.is_copied;
        let on_copy = std::mem::replace(&mut self.on_copy, Box::new(|_, _, _| {}));
        let on_click = std::mem::replace(&mut self.on_click, Box::new(|_, _, _| {}));

        let link_id = SharedString::from(format!("link-{label}"));
        let copy_id = SharedString::from(format!("copy-link-{label}"));

        v_flex()
            .group("row_group")
            .gap_1()
            .mb_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(label),
                    )
                    .child(render_copy_button(copy_id, is_copied, theme, on_copy)),
            )
            .child(
                div()
                    .id(link_id)
                    .text_xs()
                    .text_color(theme.primary)
                    .cursor(CursorStyle::PointingHand)
                    .hover(gpui::Styled::underline)
                    .on_click(move |ev, window, cx| on_click(ev, window, cx))
                    .child(text),
            )
    }
}

pub struct CollapsibleText {
    label: SharedString,
    text: SharedString,
    is_expanded: bool,
    is_copied: bool,
    show_toggle: bool,
    toggle_text: (SharedString, SharedString),
    on_toggle: ToggleHandler,
    on_copy: ClickHandler,
    on_dbl_click: Option<ClickHandler>,
}

impl CollapsibleText {
    pub fn new(
        label: impl Into<SharedString>,
        text: impl Into<SharedString>,
        is_expanded: bool,
        is_copied: bool,
        toggle_labels: (impl Into<SharedString>, impl Into<SharedString>),
        on_toggle: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
        on_copy: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            is_expanded,
            is_copied,
            show_toggle: true,
            toggle_text: (toggle_labels.0.into(), toggle_labels.1.into()),
            on_toggle: Box::new(on_toggle),
            on_copy: Box::new(on_copy),
            on_dbl_click: None,
        }
    }

    #[must_use]
    pub fn show_toggle(mut self, show: bool) -> Self {
        self.show_toggle = show;
        self
    }

    pub fn on_double_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dbl_click = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn render(mut self, theme: &Theme) -> impl IntoElement {
        let label = self.label.clone();
        let text = self.text.clone();
        let is_expanded = self.is_expanded;
        let is_copied = self.is_copied;
        let (expand_text, collapse_text) = self.toggle_text.clone();
        let show_toggle = self.show_toggle;

        let on_toggle = std::mem::replace(&mut self.on_toggle, Box::new(|_, _, _| {}));
        let on_copy = std::mem::replace(&mut self.on_copy, Box::new(|_, _, _| {}));
        let on_dbl_click = self.on_dbl_click.take();

        let content_id = SharedString::from(format!("content-{label}"));
        let toggle_id = SharedString::from(format!("toggle-{label}"));
        let copy_id = SharedString::from(format!("copy-text-{label}"));

        v_flex()
            .group("row_group")
            .gap_1()
            .mb_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(label),
                    )
                    .child(render_copy_button(copy_id, is_copied, theme, on_copy)),
            )
            .child(
                v_flex()
                    .child(
                        div()
                            .id(content_id)
                            .overflow_hidden()
                            .when_some(on_dbl_click, |el, handler| {
                                el.on_click(move |ev, window, cx| {
                                    if ev.click_count() == 2 {
                                        handler(ev, window, cx);
                                    }
                                })
                            })
                            .child(Label::new(text).text_xs()),
                    )
                    .when(show_toggle, |s| {
                        s.child(
                            div()
                                .id(toggle_id)
                                .mt_1()
                                .text_xs()
                                .text_color(theme.primary)
                                .cursor(CursorStyle::PointingHand)
                                .child(if is_expanded {
                                    collapse_text
                                } else {
                                    expand_text
                                })
                                .on_mouse_down(MouseButton::Left, move |ev, window, cx| {
                                    on_toggle(ev, window, cx);
                                }),
                        )
                    }),
            )
    }
}
