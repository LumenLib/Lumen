use crate::services::data::{SortField, SortOrder};
use crate::services::data_store::DataStore;
use crate::services::{AppViewMode, MainApp};
use crate::ui::{
    components::{FetchMode, FolderSelector, muted_input},
    icons::IconName,
    views::main_window::{Cancel, render_separator},
};
use gpui::prelude::*;
use gpui::{
    AppContext, Entity, EventEmitter, FontWeight, MouseButton, Pixels, Point, Window, div, px, rems,
};
use gpui_component::input::InputEvent;
use gpui_component::{
    ActiveTheme, Selectable, Theme,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use i18n::{I18nKey, Language, t};
use std::sync::Arc;

/// 工具栏事件
#[derive(Clone)]
pub enum ToolbarEvent {
    /// 搜索文本改变
    Search(String),
    /// 打开手动添加对话框
    OpenManualAdd,
    /// 打开抓取对话框
    OpenFetch(FetchMode),
    /// 运行重复项检测
    RunDuplicateDetection,
    /// 将选中的订阅项添加到文献库
    AddSubscriptionToLibrary,
    /// 将选中的订阅项添加到指定文件夹
    AddSubscriptionToFolder(Option<String>),
    /// 显示文件夹选择器（用于订阅添加）
    ShowFolderSelector(Point<Pixels>),
    /// 排序方式改变
    SortChanged(SortField, SortOrder),
}

impl EventEmitter<ToolbarEvent> for ToolbarView {}

/// 顶部工具栏视图
pub struct ToolbarView {
    /// 应用控制器
    pub app: Arc<MainApp>,
    pub data_store: Entity<DataStore>,
    /// 搜索输入状态
    search_input: Entity<InputState>,
    /// 文件夹选择器（用于订阅添加到文件夹）
    pub folder_selector: Option<(Entity<FolderSelector>, Point<Pixels>)>,
    /// 是否显示排序菜单
    show_sort_menu: bool,
    /// 是否显示添加文献菜单
    show_add_menu: bool,
}

impl ToolbarView {
    pub fn new(
        app: Arc<MainApp>,
        data_store: Entity<DataStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let lang = app.current_language();
        let search_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t(I18nKey::SearchBoxPlaceholder, lang))
        });

        // 订阅搜索输入事件
        cx.subscribe(
            &search_input,
            |_, input_state: Entity<InputState>, event, cx| {
                if let InputEvent::Change = event {
                    let query = input_state.read(cx).text().to_string();
                    cx.emit(ToolbarEvent::Search(query));
                }
            },
        )
        .detach();

        Self {
            app,
            data_store,
            search_input,
            folder_selector: None,
            show_sort_menu: false,
            show_add_menu: false,
        }
    }

    fn toggle_sort_menu(&mut self, cx: &mut Context<Self>) {
        self.show_sort_menu = !self.show_sort_menu;
        cx.notify();
    }

    fn toggle_add_menu(&mut self, cx: &mut Context<Self>) {
        self.show_add_menu = !self.show_add_menu;
        cx.notify();
    }

    fn change_sort(&mut self, field: SortField, order: SortOrder, cx: &mut Context<Self>) {
        cx.emit(ToolbarEvent::SortChanged(field, order));
        self.show_sort_menu = false;
        cx.notify();
    }

    /// 刷新搜索框占位符
    pub fn refresh_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lang = self.app.current_language();

        self.search_input.update(cx, |state, cx| {
            state.set_placeholder(t(I18nKey::SearchBoxPlaceholder, lang), window, cx);
        });
    }

    fn render_sort_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.show_sort_menu {
            return None;
        }

        let lang = self.app.current_language();

        let ui = cx.global::<crate::services::ui_state::UiState>();
        let (current_field, current_order) = (ui.sort_field, ui.sort_order);

        Some(
            div()
                .absolute()
                .top(rems(2.25))
                .right(rems(1.0))
                .w(rems(12.5))
                .bg(theme.background)
                .text_color(theme.popover_foreground)
                .rounded(rems(0.5))
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .p_2()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.show_sort_menu = false;
                    cx.notify();
                }))
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.muted_foreground)
                                .child(t(I18nKey::SortBy, lang)),
                        )
                        .child(self.render_sort_field_option(
                            SortField::Title,
                            I18nKey::SortByTitle,
                            current_field,
                            current_order,
                            theme,
                            lang,
                            cx,
                        ))
                        .child(self.render_sort_field_option(
                            SortField::Author,
                            I18nKey::SortByAuthor,
                            current_field,
                            current_order,
                            theme,
                            lang,
                            cx,
                        ))
                        .child(self.render_sort_field_option(
                            SortField::Year,
                            I18nKey::SortByYear,
                            current_field,
                            current_order,
                            theme,
                            lang,
                            cx,
                        ))
                        .child(self.render_sort_field_option(
                            SortField::Journal,
                            I18nKey::SortByJournal,
                            current_field,
                            current_order,
                            theme,
                            lang,
                            cx,
                        ))
                        .child(render_separator(theme))
                        .child(self.render_sort_order_option(
                            SortOrder::Ascending,
                            I18nKey::SortAscending,
                            current_field,
                            current_order,
                            theme,
                            lang,
                            cx,
                        ))
                        .child(self.render_sort_order_option(
                            SortOrder::Descending,
                            I18nKey::SortDescending,
                            current_field,
                            current_order,
                            theme,
                            lang,
                            cx,
                        )),
                ),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_sort_field_option(
        &self,
        field: SortField,
        label_key: I18nKey,
        current_field: SortField,
        current_order: SortOrder,
        theme: &Theme,
        lang: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = current_field == field;

        div()
            .px_2()
            .py_1p5()
            .rounded(rems(0.25))
            .when(is_selected, |this| {
                this.bg(theme.accent).text_color(theme.accent_foreground)
            })
            .when(!is_selected, |this| {
                this.hover(|this| this.bg(theme.muted))
                    .text_color(theme.popover_foreground)
            })
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.change_sort(field, current_order, cx);
                }),
            )
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(t(label_key, lang)))
                    .when(is_selected, |this| {
                        this.child(div().text_xs().child(match current_order {
                            SortOrder::Ascending => "↑",
                            SortOrder::Descending => "↓",
                        }))
                    }),
            )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_sort_order_option(
        &self,
        order: SortOrder,
        label_key: I18nKey,
        current_field: SortField,
        current_order: SortOrder,
        theme: &Theme,
        lang: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = current_order == order;

        div()
            .px_2()
            .py_1p5()
            .rounded(rems(0.25))
            .when(is_selected, |this| {
                this.bg(theme.accent).text_color(theme.accent_foreground)
            })
            .when(!is_selected, |this| {
                this.hover(|this| this.bg(theme.muted))
                    .text_color(theme.popover_foreground)
            })
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.change_sort(current_field, order, cx);
                }),
            )
            .child(div().text_sm().child(t(label_key, lang)))
    }

    fn render_add_menu(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.show_add_menu {
            return None;
        }

        let lang = self.app.current_language();

        Some(
            div()
                .absolute()
                .top(rems(2.25))
                .right(rems(3.5))
                .w(rems(11.25))
                .bg(theme.background)
                .text_color(theme.popover_foreground)
                .rounded(rems(0.5))
                .border_1()
                .border_color(theme.border)
                .shadow_md()
                .p_2()
                .occlude()
                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                    this.show_add_menu = false;
                    cx.notify();
                }))
                .child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .px_2()
                                .py_1p5()
                                .rounded(rems(0.25))
                                .hover(|this| this.bg(theme.muted))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.emit(ToolbarEvent::OpenManualAdd);
                                        this.show_add_menu = false;
                                        cx.notify();
                                    }),
                                )
                                .child(div().text_sm().child(t(I18nKey::ManualAdd, lang))),
                        )
                        .child(render_separator(theme))
                        .child(
                            div()
                                .px_2()
                                .py_1p5()
                                .rounded(rems(0.25))
                                .hover(|this| this.bg(theme.muted))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.emit(ToolbarEvent::OpenFetch(FetchMode::BibTeX));
                                        this.show_add_menu = false;
                                        cx.notify();
                                    }),
                                )
                                .child(div().text_sm().child(t(I18nKey::BibTeXImport, lang))),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1p5()
                                .rounded(rems(0.25))
                                .hover(|this| this.bg(theme.muted))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.emit(ToolbarEvent::OpenFetch(FetchMode::Doi));
                                        this.show_add_menu = false;
                                        cx.notify();
                                    }),
                                )
                                .child(div().text_sm().child(t(I18nKey::DoiImport, lang))),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1p5()
                                .rounded(rems(0.25))
                                .hover(|this| this.bg(theme.muted))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.emit(ToolbarEvent::OpenFetch(FetchMode::ArXiv));
                                        this.show_add_menu = false;
                                        cx.notify();
                                    }),
                                )
                                .child(div().text_sm().child(t(I18nKey::ArXivImport, lang))),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1p5()
                                .rounded(rems(0.25))
                                .hover(|this| this.bg(theme.muted))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.emit(ToolbarEvent::OpenFetch(FetchMode::Dblp));
                                        this.show_add_menu = false;
                                        cx.notify();
                                    }),
                                )
                                .child(div().text_sm().child(t(I18nKey::DblpSearch, lang))),
                        )
                        .child(
                            div()
                                .px_2()
                                .py_1p5()
                                .rounded(rems(0.25))
                                .hover(|this| this.bg(theme.muted))
                                .cursor_pointer()
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        cx.emit(ToolbarEvent::OpenFetch(FetchMode::OpenAlex));
                                        this.show_add_menu = false;
                                        cx.notify();
                                    }),
                                )
                                .child(div().text_sm().child("OpenAlex")),
                        ),
                ),
        )
    }

    /// 工具栏横条（不含下拉菜单）
    pub fn render_bar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (view_mode, _, show_sub_add) = {
            let ui = cx.global::<crate::services::ui_state::UiState>();
            let show_sub_add =
                if ui.view_mode == AppViewMode::Subscription {
                    self.data_store.read(cx).feed_items.iter().any(|s| {
                        ui.selected_feed_item_ids.contains(&s.id) && !s.is_added_to_library
                    })
                } else {
                    false
                };
            (ui.view_mode, (), show_sub_add)
        };

        div()
            .w_full()
            .h(rems(2.5))
            .flex_shrink_0()
            .on_action(cx.listener(|_, _: &Cancel, _, cx| {
                cx.notify();
            }))
            .child(
                h_flex()
                    .size_full()
                    .bg(cx.theme().background)
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .px_4()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex().w(rems(18.75)).child(
                            div()
                                .id("search-input-wrapper")
                                .flex_grow(1.0)
                                .h_full()
                                .bg(cx.theme().background)
                                .child(
                                    muted_input(Input::new(&self.search_input), cx.theme())
                                        .w_full(),
                                ),
                        ),
                    )
                    .child(div().flex_grow(1.0).h_full())
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .when(view_mode == AppViewMode::Library, |this| {
                                this.child(
                                    Button::new("sort-trigger")
                                        .icon(IconName::ArrowUpDown)
                                        .ghost()
                                        .selected(self.show_sort_menu)
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_sort_menu(cx);
                                        })),
                                )
                                .child(
                                    Button::new("find-duplicates-trigger")
                                        .icon(IconName::Clear)
                                        .ghost()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(|_this, _, _, cx| {
                                            cx.emit(ToolbarEvent::RunDuplicateDetection);
                                        })),
                                )
                                .child(
                                    Button::new("add-literature")
                                        .icon(IconName::Add)
                                        .ghost()
                                        .selected(self.show_add_menu)
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_add_menu(cx);
                                        })),
                                )
                            })
                            .when(show_sub_add, |this| {
                                this.child(
                                    Button::new("add-selection-to-library")
                                        .icon(IconName::Add)
                                        .ghost()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(
                                            |_this, event: &gpui::ClickEvent, _, cx| {
                                                let pos = Point::new(
                                                    event.position().x,
                                                    event.position().y + px(30.0),
                                                );
                                                cx.emit(ToolbarEvent::ShowFolderSelector(pos));
                                            },
                                        )),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 下拉菜单（需由父级在内容区之后渲染，确保覆盖内容区）
    pub fn render_dropdowns(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let theme = cx.theme().clone();
        let mut children = Vec::new();
        if let Some(menu) = self.render_sort_menu(&theme, cx) {
            children.push(menu.into_any_element());
        }
        if let Some(menu) = self.render_add_menu(&theme, cx) {
            children.push(menu.into_any_element());
        }
        children
    }
}

impl Render for ToolbarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut children: Vec<gpui::AnyElement> =
            vec![self.render_bar(window, cx).into_any_element()];
        children.extend(self.render_dropdowns(cx));
        div().relative().w_full().h(rems(2.5)).children(children)
    }
}
