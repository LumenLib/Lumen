use gpui::prelude::*;
use gpui::{Half, Pixels, Point, Window, div, px};
use gpui_component::ActiveTheme;

/// 文献拖拽信息
/// 用于在拖拽过程中传递文献 ID 列表
#[derive(Clone, Debug)]
pub struct LiteratureDragInfo {
    /// 被拖拽的文献 ID 列表
    pub literature_ids: Vec<String>,
    /// 拖拽时的鼠标位置
    pub position: Point<Pixels>,
}

impl LiteratureDragInfo {
    /// 创建新的拖拽信息
    #[must_use]
    pub fn new(literature_ids: Vec<String>) -> Self {
        Self {
            literature_ids,
            position: Point::default(),
        }
    }

    /// 更新位置
    #[must_use]
    pub fn with_position(mut self, pos: Point<Pixels>) -> Self {
        self.position = pos;
        self
    }

    /// 获取文献数量
    #[must_use]
    pub fn count(&self) -> usize {
        self.literature_ids.len()
    }
}

impl Render for LiteratureDragInfo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let count = self.count();

        // 简约的拖拽预览：数量徽章
        let badge_size = gpui::size(px(24.0), px(24.0));

        // 使用 padding 来定位，让徽章中心对准鼠标位置
        div()
            .pl(self.position.x - badge_size.width.half())
            .pt(self.position.y - badge_size.height.half())
            .child(
                div()
                    .size(badge_size.width)
                    .rounded_full()
                    .bg(theme.primary)
                    .shadow_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.primary_foreground)
                            .child(count.to_string()),
                    ),
            )
    }
}

/// 文件夹拖拽信息
#[derive(Clone, Debug)]
pub struct FolderDragInfo {
    pub folder_id: String,
    pub folder_name: String,
    pub position: Point<Pixels>,
}

impl FolderDragInfo {
    #[must_use]
    pub fn new(folder_id: String, folder_name: String) -> Self {
        Self {
            folder_id,
            folder_name,
            position: Point::default(),
        }
    }

    #[must_use]
    pub fn with_position(mut self, pos: Point<Pixels>) -> Self {
        self.position = pos;
        self
    }
}

impl Render for FolderDragInfo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let theme = cx.theme().clone();

        div().pl(self.position.x).pt(self.position.y).child(
            div()
                .px_3()
                .py_1()
                .bg(theme.background)
                .border_1()
                .border_color(theme.border)
                .shadow_md()
                .rounded_md()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(self.folder_name.clone()),
                ),
        )
    }
}
