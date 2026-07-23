use crate::app_state::data::DataStore;
use crate::ui::components::{CollapsibleText, DetailRow, LinkRow};
use components::IconName;
use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, AppContext, AsyncApp, Entity, FontWeight, WeakEntity, Window, div, rems,
};
use gpui_component::{ActiveTheme, Icon, h_flex, label::Label, v_flex};
use i18n::{I18nKey, t, tf};
use models::FeedItem;
use parser::normalize::author_full_name;
use services::app::MainApp;
use std::sync::Arc;

/// 订阅详情视图的状态 (Buffer)
#[derive(Clone)]
struct DetailState {
    /// 当前选中的 ID 列表（用于变更检测）
    selected_ids: Vec<String>,
    /// 渲染模式
    mode: DetailMode,
}

#[derive(Clone)]
enum DetailMode {
    /// 无选中
    None,
    /// 选中多个
    Multiple(usize),
    /// 选中单个并预处理好数据
    Single(Box<SingleDetailBuffer>),
}

/// 单个订阅条目的渲染缓冲数据
#[derive(Clone)]
struct SingleDetailBuffer {
    item: Arc<FeedItem>,
    authors_text: String,
    abstract_display: String,
}

/// 订阅详情视图
pub struct SubscriptionDetailView {
    app: Arc<MainApp>,
    data_store: Entity<DataStore>,
    /// 摘要是否展开
    abstract_expanded: bool,
    /// 预实体化缓冲状态
    state: DetailState,
    /// 当前已复制的字段 ID
    copied_field: Option<String>,
}

impl SubscriptionDetailView {
    pub fn new(app: Arc<MainApp>, data_store: Entity<DataStore>) -> Self {
        Self {
            app,
            data_store,
            abstract_expanded: false,
            state: DetailState {
                selected_ids: Vec::new(),
                mode: DetailMode::None,
            },
            copied_field: None,
        }
    }

    /// 同步并预实体化状态 (Buffer Update)
    fn sync_state(&mut self, cx: &mut Context<Self>) {
        let data = self.data_store.read(cx);
        let ui = cx.global::<crate::app_state::ui::UiState>();

        // 1. 变更检测
        let current_selected: Vec<String> = ui.selected_feed_item_ids.iter().cloned().collect();

        // 检查选中项是否变化
        let ids_changed = self.state.selected_ids != current_selected;

        if !ids_changed {
            return;
        }

        self.state.selected_ids = current_selected;

        let selected_count = self.state.selected_ids.len();
        if selected_count == 0 {
            self.state.mode = DetailMode::None;
        } else if selected_count > 1 {
            self.state.mode = DetailMode::Multiple(selected_count);
        } else {
            // 2. 预实体化单个订阅条目数据
            let first_id = self
                .state
                .selected_ids
                .first()
                .expect("selected_ids is non-empty");
            if let Some(item) = data.feed_items.iter().find(|s| s.id == *first_id) {
                let item = item.clone();

                // 预处理作者
                let authors_text = item
                    .authors
                    .iter()
                    .map(author_full_name)
                    .collect::<Vec<_>>()
                    .join(", ");

                // 预处理摘要显示（截断逻辑）
                let abstract_display = if let Some(ref text) = item.abstract_text {
                    if !self.abstract_expanded && text.chars().count() > 30 {
                        let mut truncated = text.chars().take(30).collect::<String>();
                        truncated.push_str("...");
                        truncated
                    } else {
                        text.clone()
                    }
                } else {
                    String::new()
                };

                self.state.mode = DetailMode::Single(Box::new(SingleDetailBuffer {
                    item,
                    authors_text,
                    abstract_display,
                }));
            } else {
                self.state.mode = DetailMode::None;
            }
        }

        cx.notify();
    }

    fn toggle_abstract(&mut self, cx: &mut Context<Self>) {
        self.abstract_expanded = !self.abstract_expanded;
        // 摘要状态改变后，需要重新计算截断
        if let DetailMode::Single(ref mut buffer) = self.state.mode
            && let Some(ref text) = buffer.item.abstract_text
        {
            buffer.abstract_display = if !self.abstract_expanded && text.chars().count() > 30 {
                let mut truncated = text.chars().take(30).collect::<String>();
                truncated.push_str("...");
                truncated
            } else {
                text.clone()
            };
        }
        cx.notify();
    }
    fn copy_text(
        &mut self,
        text: String,
        field_id: String,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.copied_field = Some(field_id);
        cx.notify();

        cx.spawn(move |view: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(1500))
                    .await;
                let _ = cx.update_window(window, |_, _, cx| {
                    let _ = view.update(cx, |this, cx| {
                        this.copied_field = None;
                        cx.notify();
                    });
                });
            }
        })
        .detach();
    }
}

impl Render for SubscriptionDetailView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_state(cx);

        let theme = cx.theme().clone();
        let lang = self.app.current_language();

        match &self.state.mode {
            DetailMode::None => div()
                .id("subscription-detail-empty")
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .bg(theme.background)
                .child(t(I18nKey::NoSubscriptionSelected, lang)),
            DetailMode::Multiple(count) => div()
                .id("subscription-detail-multiple")
                .size_full()
                .bg(theme.background)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .child(
                    Icon::new(IconName::Bell)
                        .size(rems(3.0))
                        .text_color(theme.muted_foreground),
                )
                .child(div().text_lg().text_color(theme.foreground).child(tf(
                    I18nKey::SelectedSubscriptionCount,
                    lang,
                    &[&count.to_string()],
                ))),
            DetailMode::Single(buffer) => {
                let item = &buffer.item;
                div()
                    .id("sub-detail-container")
                    .flex()
                    .flex_col()
                    .size_full()
                    .bg(theme.background)
                    .border_l_1()
                    .border_color(theme.border)
                    .px_3()
                    .py_3()
                    .overflow_y_scroll()
                    .child(
                        v_flex()
                            .group("row_group")
                            .items_start()
                            .gap_1()
                            .child(
                                Label::new(item.title.clone())
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.foreground)
                                    .line_clamp(10),
                            )
                            .child(crate::ui::components::detail_widgets::render_copy_button(
                                "copy-title",
                                self.copied_field.as_ref() == Some(&"title".to_string()),
                                &theme,
                                cx.listener({
                                    let title = item.title.clone();
                                    move |this, _, window, cx| {
                                        this.copy_text(
                                            title.clone(),
                                            "title".to_string(),
                                            window.window_handle(),
                                            cx,
                                        );
                                    }
                                }),
                            )),
                    )
                    .when_some(item.journal.clone(), |this, journal| {
                        if journal.trim().is_empty() {
                            this
                        } else {
                            this.child(
                                DetailRow::new(
                                    t(I18nKey::JournalPlaceholder, lang),
                                    journal.clone(),
                                    self.copied_field.as_ref() == Some(&"journal".to_string()),
                                    cx.listener({
                                        let val = journal.clone();
                                        move |this, _, window, cx| {
                                            this.copy_text(
                                                val.clone(),
                                                "journal".to_string(),
                                                window.window_handle(),
                                                cx,
                                            );
                                        }
                                    }),
                                )
                                .render(&theme),
                            )
                        }
                    })
                    .child(h_flex().gap_4().when_some(item.year, |this, year| {
                        let year_str = year.to_string();
                        this.child(
                            DetailRow::new(
                                t(I18nKey::Year, lang),
                                year_str.clone(),
                                self.copied_field.as_ref() == Some(&"year".to_string()),
                                cx.listener({
                                    let val = year_str.clone();
                                    move |this, _, window, cx| {
                                        this.copy_text(
                                            val.clone(),
                                            "year".to_string(),
                                            window.window_handle(),
                                            cx,
                                        );
                                    }
                                }),
                            )
                            .render(&theme),
                        )
                    }))
                    .child(
                        h_flex()
                            .gap_4()
                            .when_some(item.volume.clone(), |this, vol| {
                                this.child(
                                    DetailRow::new(
                                        "Vol.",
                                        vol.clone(),
                                        self.copied_field.as_ref() == Some(&"vol".to_string()),
                                        cx.listener({
                                            let val = vol.clone();
                                            move |this, _, window, cx| {
                                                this.copy_text(
                                                    val.clone(),
                                                    "vol".to_string(),
                                                    window.window_handle(),
                                                    cx,
                                                );
                                            }
                                        }),
                                    )
                                    .render(&theme),
                                )
                            })
                            .when_some(item.issue.clone(), |this, iss| {
                                this.child(
                                    DetailRow::new(
                                        "No.",
                                        iss.clone(),
                                        self.copied_field.as_ref() == Some(&"issue".to_string()),
                                        cx.listener({
                                            let val = iss.clone();
                                            move |this, _, window, cx| {
                                                this.copy_text(
                                                    val.clone(),
                                                    "issue".to_string(),
                                                    window.window_handle(),
                                                    cx,
                                                );
                                            }
                                        }),
                                    )
                                    .render(&theme),
                                )
                            })
                            .when_some(item.pages.clone(), |this, pag| {
                                this.child(
                                    DetailRow::new(
                                        "Pages",
                                        pag.clone(),
                                        self.copied_field.as_ref() == Some(&"pages".to_string()),
                                        cx.listener({
                                            let val = pag.clone();
                                            move |this, _, window, cx| {
                                                this.copy_text(
                                                    val.clone(),
                                                    "pages".to_string(),
                                                    window.window_handle(),
                                                    cx,
                                                );
                                            }
                                        }),
                                    )
                                    .render(&theme),
                                )
                            }),
                    )
                    .when(!buffer.authors_text.is_empty(), |this| {
                        let authors = buffer.authors_text.clone();
                        this.child(
                            DetailRow::new(
                                t(I18nKey::Authors, lang),
                                authors.clone(),
                                self.copied_field.as_ref() == Some(&"authors".to_string()),
                                cx.listener({
                                    let val = authors.clone();
                                    move |this, _, window, cx| {
                                        this.copy_text(
                                            val.clone(),
                                            "authors".to_string(),
                                            window.window_handle(),
                                            cx,
                                        );
                                    }
                                }),
                            )
                            .render(&theme),
                        )
                    })
                    .when_some(item.doi.clone(), |s, doi| {
                        if doi.trim().is_empty() {
                            s
                        } else {
                            let val = doi.clone();
                            s.child(
                                DetailRow::new(
                                    "DOI",
                                    val.clone(),
                                    self.copied_field.as_ref() == Some(&"doi".to_string()),
                                    cx.listener({
                                        let val = val.clone();
                                        move |this, _, window, cx| {
                                            this.copy_text(
                                                val.clone(),
                                                "doi".to_string(),
                                                window.window_handle(),
                                                cx,
                                            );
                                        }
                                    }),
                                )
                                .render(&theme),
                            )
                        }
                    })
                    .when_some(item.url.clone(), |s, url| {
                        if url.trim().is_empty() {
                            s
                        } else {
                            let val = url.clone();
                            s.child(
                                LinkRow::new(
                                    "URL",
                                    val.clone(),
                                    self.copied_field.as_ref() == Some(&"url".to_string()),
                                    cx.listener({
                                        let val = val.clone();
                                        move |this, _, window, cx| {
                                            this.copy_text(
                                                val.clone(),
                                                "url".to_string(),
                                                window.window_handle(),
                                                cx,
                                            );
                                        }
                                    }),
                                    cx.listener({
                                        let val = val.clone();
                                        move |_, _, _, _| {
                                            #[cfg(target_os = "macos")]
                                            let _ = std::process::Command::new("open")
                                                .arg(&val)
                                                .spawn();
                                            #[cfg(target_os = "windows")]
                                            {
                                                use std::os::windows::process::CommandExt;
                                                let _ = std::process::Command::new("cmd")
                                                    .arg("/c")
                                                    .arg("start")
                                                    .arg("")
                                                    .arg(&val)
                                                    .creation_flags(0x08000000)
                                                    .spawn();
                                            }
                                            #[cfg(target_os = "linux")]
                                            let _ = std::process::Command::new("xdg-open")
                                                .arg(&val)
                                                .spawn();
                                        }
                                    }),
                                )
                                .render(&theme),
                            )
                        }
                    })
                    .child(
                        DetailRow::new(
                            t(I18nKey::UpdatedAt, lang),
                            item.published_at.clone().unwrap_or(item.added_at.clone()),
                            self.copied_field.as_ref() == Some(&"updated_at".to_string()),
                            cx.listener({
                                let val =
                                    item.published_at.clone().unwrap_or(item.added_at.clone());
                                move |this, _, window, cx| {
                                    this.copy_text(
                                        val.clone(),
                                        "updated_at".to_string(),
                                        window.window_handle(),
                                        cx,
                                    );
                                }
                            }),
                        )
                        .render(&theme),
                    )
                    .child(
                        CollapsibleText::new(
                            t(I18nKey::Abstract, lang),
                            if buffer.abstract_display.is_empty() {
                                t(I18nKey::NoAbstract, lang).to_string()
                            } else {
                                buffer.abstract_display.clone()
                            },
                            self.abstract_expanded,
                            self.copied_field.as_ref() == Some(&"abstract".to_string()),
                            (t(I18nKey::Expand, lang), t(I18nKey::Collapse, lang)),
                            cx.listener(|this, _, _window, cx| {
                                this.toggle_abstract(cx);
                            }),
                            cx.listener({
                                let val = item.abstract_text.clone().unwrap_or_default();
                                move |this, _, window, cx| {
                                    this.copy_text(
                                        val.clone(),
                                        "abstract".to_string(),
                                        window.window_handle(),
                                        cx,
                                    );
                                }
                            }),
                        )
                        .on_double_click({
                            let val = item.abstract_text.clone().unwrap_or_default();
                            move |_, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(val.clone()));
                            }
                        })
                        .show_toggle(item.abstract_text.is_some())
                        .render(&theme),
                    )
            }
        }
    }
}
