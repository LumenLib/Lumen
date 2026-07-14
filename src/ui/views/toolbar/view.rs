use crate::services::data::{SortField, SortOrder};
use crate::services::data_store::DataStore;
use crate::services::{AppViewMode, MainApp};
use crate::ui::{
    components::{FetchMode, FolderSelector, muted_input},
    icons::IconName,
    views::main_window::Cancel,
};
use gpui::prelude::*;
use gpui::{
    AppContext, DismissEvent, Entity, EventEmitter, MouseButton, Pixels, Point, Window, div, px,
    rems,
};
use gpui_component::input::InputEvent;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use gpui_component::{
    ActiveTheme, Selectable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
};
use i18n::{I18nKey, t};
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
    /// 排序菜单（PopupMenu 实现）
    sort_menu: Option<Entity<PopupMenu>>,
    /// 添加文献菜单（PopupMenu 实现）
    pub add_menu: Option<Entity<PopupMenu>>,
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
            sort_menu: None,
            add_menu: None,
        }
    }

    fn toggle_sort_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sort_menu.is_some() {
            self.sort_menu = None;
            cx.notify();
            return;
        }

        let lang = self.app.current_language();
        let view_weak = cx.entity().downgrade();

        let ui = cx.global::<crate::services::ui_state::UiState>();
        let (current_field, current_order) = (ui.sort_field, ui.sort_order);
        let _ = ui;

        let menu = PopupMenu::build(window, cx, move |mut menu, window, _cx| {
            let width: Pixels = rems(12.5).to_pixels(window.rem_size());
            menu = menu.min_w(width);

            menu = menu.label(t(I18nKey::SortBy, lang));
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::SortByTitle, lang))
                    .checked(current_field == SortField::Title)
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |this, cx| {
                                    cx.emit(ToolbarEvent::SortChanged(
                                        SortField::Title,
                                        current_order,
                                    ));
                                    this.sort_menu = None;
                                    cx.notify();
                                });
                            }
                        }
                    }),
            );
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::SortByAuthor, lang))
                    .checked(current_field == SortField::Author)
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |this, cx| {
                                    cx.emit(ToolbarEvent::SortChanged(
                                        SortField::Author,
                                        current_order,
                                    ));
                                    this.sort_menu = None;
                                    cx.notify();
                                });
                            }
                        }
                    }),
            );
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::SortByYear, lang))
                    .checked(current_field == SortField::Year)
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |this, cx| {
                                    cx.emit(ToolbarEvent::SortChanged(
                                        SortField::Year,
                                        current_order,
                                    ));
                                    this.sort_menu = None;
                                    cx.notify();
                                });
                            }
                        }
                    }),
            );
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::SortByJournal, lang))
                    .checked(current_field == SortField::Journal)
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |this, cx| {
                                    cx.emit(ToolbarEvent::SortChanged(
                                        SortField::Journal,
                                        current_order,
                                    ));
                                    this.sort_menu = None;
                                    cx.notify();
                                });
                            }
                        }
                    }),
            );
            menu = menu.separator();
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::SortAscending, lang))
                    .checked(current_order == SortOrder::Ascending)
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |this, cx| {
                                    cx.emit(ToolbarEvent::SortChanged(
                                        current_field,
                                        SortOrder::Ascending,
                                    ));
                                    this.sort_menu = None;
                                    cx.notify();
                                });
                            }
                        }
                    }),
            );
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::SortDescending, lang))
                    .checked(current_order == SortOrder::Descending)
                    .on_click({
                        let view_weak = view_weak.clone();
                        move |_, _, cx| {
                            if let Some(view) = view_weak.upgrade() {
                                view.update(cx, |this, cx| {
                                    cx.emit(ToolbarEvent::SortChanged(
                                        current_field,
                                        SortOrder::Descending,
                                    ));
                                    this.sort_menu = None;
                                    cx.notify();
                                });
                            }
                        }
                    }),
            );
            menu
        });

        cx.subscribe(&menu, |this: &mut ToolbarView, _, _: &DismissEvent, cx| {
            this.sort_menu = None;
            cx.notify();
        })
        .detach();

        self.sort_menu = Some(menu);
        cx.notify();
    }

    fn toggle_add_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.add_menu.is_some() {
            self.add_menu = None;
            cx.notify();
            return;
        }

        let lang = self.app.current_language();
        let view_weak = cx.entity().downgrade();

        let menu = PopupMenu::build(window, cx, move |mut menu, window, _cx| {
            let width: Pixels = rems(11.25).to_pixels(window.rem_size());
            menu = menu.min_w(width);

            menu = menu.item(PopupMenuItem::new(t(I18nKey::ManualAdd, lang)).on_click({
                let view_weak = view_weak.clone();
                move |_, _, cx| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_, cx| {
                            cx.emit(ToolbarEvent::OpenManualAdd);
                        });
                    }
                }
            }));
            menu = menu.separator();
            menu = menu.item(
                PopupMenuItem::new(t(I18nKey::BibTeXImport, lang)).on_click({
                    let view_weak = view_weak.clone();
                    move |_, _, cx| {
                        if let Some(view) = view_weak.upgrade() {
                            view.update(cx, |_, cx| {
                                cx.emit(ToolbarEvent::OpenFetch(FetchMode::BibTeX));
                            });
                        }
                    }
                }),
            );
            menu = menu.item(PopupMenuItem::new(t(I18nKey::DoiImport, lang)).on_click({
                let view_weak = view_weak.clone();
                move |_, _, cx| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_, cx| {
                            cx.emit(ToolbarEvent::OpenFetch(FetchMode::Doi));
                        });
                    }
                }
            }));
            menu = menu.item(PopupMenuItem::new(t(I18nKey::ArXivImport, lang)).on_click({
                let view_weak = view_weak.clone();
                move |_, _, cx| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_, cx| {
                            cx.emit(ToolbarEvent::OpenFetch(FetchMode::ArXiv));
                        });
                    }
                }
            }));
            menu = menu.item(PopupMenuItem::new(t(I18nKey::DblpSearch, lang)).on_click({
                let view_weak = view_weak.clone();
                move |_, _, cx| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_, cx| {
                            cx.emit(ToolbarEvent::OpenFetch(FetchMode::Dblp));
                        });
                    }
                }
            }));
            menu = menu.item(PopupMenuItem::new("OpenAlex").on_click({
                let view_weak = view_weak.clone();
                move |_, _, cx| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |_, cx| {
                            cx.emit(ToolbarEvent::OpenFetch(FetchMode::OpenAlex));
                        });
                    }
                }
            }));
            menu
        });

        cx.subscribe(&menu, |this: &mut ToolbarView, _, _: &DismissEvent, cx| {
            this.add_menu = None;
            cx.notify();
        })
        .detach();

        self.add_menu = Some(menu);
        cx.notify();
    }

    /// 刷新搜索框占位符
    pub fn refresh_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lang = self.app.current_language();

        self.search_input.update(cx, |state, cx| {
            state.set_placeholder(t(I18nKey::SearchBoxPlaceholder, lang), window, cx);
        });
    }

    fn render_sort_menu(&self, _cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.sort_menu.as_ref().map(|menu| {
            div()
                .absolute()
                .top(rems(2.25))
                .right(rems(1.0))
                .child(menu.clone())
        })
    }

    fn render_add_menu(&self, _cx: &mut Context<Self>) -> Option<impl IntoElement> {
        self.add_menu.as_ref().map(|menu| {
            div()
                .absolute()
                .top(rems(2.25))
                .right(rems(3.5))
                .child(menu.clone())
        })
    }

    /// 工具栏横条（不含下拉菜单）
    pub fn render_bar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let (view_mode, show_sub_add, has_selected_id) = {
            let ui = cx.global::<crate::services::ui_state::UiState>();
            let show_sub_add =
                if ui.view_mode == AppViewMode::Subscription {
                    self.data_store.read(cx).feed_items.iter().any(|s| {
                        ui.selected_feed_item_ids.contains(&s.id) && !s.is_added_to_library
                    })
                } else {
                    false
                };
            let has_selected_id = if ui.view_mode == AppViewMode::Library {
                !ui.selected_literature_ids.is_empty()
            } else {
                !ui.selected_feed_item_ids.is_empty()
            };
            (ui.view_mode, show_sub_add, has_selected_id)
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
                            .gap_2()
                            .items_center()
                            .when(view_mode == AppViewMode::Library, |this| {
                                this.child(
                                    Button::new("sort-trigger")
                                        .icon(IconName::ArrowUpDown)
                                        .ghost()
                                        .h(rems(1.5))
                                        .w(rems(1.5))
                                        .selected(self.sort_menu.is_some())
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.toggle_sort_menu(window, cx);
                                        })),
                                )
                                .child(
                                    Button::new("find-duplicates-trigger")
                                        .icon(IconName::Clear)
                                        .ghost()
                                        .h(rems(1.5))
                                        .w(rems(1.5))
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
                                        .h(rems(1.5))
                                        .w(rems(1.5))
                                        .selected(self.add_menu.is_some())
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.toggle_add_menu(window, cx);
                                        })),
                                )
                            })
                            .when(show_sub_add, |this| {
                                this.child(
                                    Button::new("add-selection-to-library")
                                        .icon(IconName::Add)
                                        .ghost()
                                        .h(rems(1.5))
                                        .w(rems(1.5))
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
                            })
                            .when(has_selected_id, |this| {
                                this.child(
                                    Button::new("close-details-panel")
                                        .icon(IconName::Close)
                                        .ghost()
                                        .h(rems(1.5))
                                        .w(rems(1.5))
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            crate::services::ui_state::UiState::update(
                                                cx,
                                                |state| {
                                                    state.selected_literature_ids.clear();
                                                    state.selected_feed_item_ids.clear();
                                                },
                                            );
                                        })),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 下拉菜单（需由父级在内容区之后渲染，确保覆盖内容区）
    pub fn render_dropdowns(&self, cx: &mut Context<Self>) -> Vec<gpui::AnyElement> {
        let mut children = Vec::new();
        if let Some(menu) = self.render_sort_menu(cx) {
            children.push(menu.into_any_element());
        }
        if let Some(menu) = self.render_add_menu(cx) {
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
