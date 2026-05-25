use gpui::prelude::*;
use gpui::{App, DefiniteLength, Entity, SharedString, Window, div};
use gpui_component::{
    ActiveTheme,
    input::{Input, InputState},
    v_flex,
};

#[derive(IntoElement)]
pub struct LabeledInput {
    label: SharedString,
    input: Entity<InputState>,
    width: Option<DefiniteLength>,
}

impl LabeledInput {
    pub fn new(label: impl Into<SharedString>, input: &Entity<InputState>) -> Self {
        Self {
            label: label.into(),
            input: input.clone(),
            width: None,
        }
    }

    pub fn width(mut self, width: impl Into<DefiniteLength>) -> Self {
        self.width = Some(width.into());
        self
    }
}

impl RenderOnce for LabeledInput {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();

        let mut container = v_flex().gap_1();

        if let Some(w) = self.width {
            container = container.w(w);
        }

        container
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground) // 统一使用柔和的前景色作为标签
                    .child(self.label),
            )
            .child(Input::new(&self.input))
    }
}
