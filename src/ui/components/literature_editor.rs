use super::{LabeledInput, muted_input, muted_select};
use crate::services::MainApp;
use crate::ui::icons::IconName;
use components::add_drag_behavior;
use database::constructors::*;
use gpui::prelude::*;
use gpui::{AppContext, Entity, FontWeight, SharedString, Window, div, rems};
use gpui_component::{
    ActiveTheme, Icon, InteractiveElementExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    scroll::ScrollableElement,
    select::{Select, SelectEvent, SelectState},
    v_flex,
};
use i18n::LiteratureTypeExt;
use i18n::{I18nKey, t};
use log::{debug, info};
use models::{Literature, LiteratureType, PublicationType};
use parser::normalize::*;
use std::sync::Arc;

#[derive(Clone)]
struct LiteratureTypeItem {
    lit_type: LiteratureType,
    title: SharedString,
}

impl gpui_component::select::SelectItem for LiteratureTypeItem {
    type Value = LiteratureType;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.lit_type
    }
}

pub type LiteratureEditorCallback =
    Box<dyn Fn(Option<Literature>, &mut Window, &mut Context<LiteratureEditor>) + Send + Sync>;

/// 文献编辑器组件 (用于手动添加、编辑及导入核对)
pub struct LiteratureEditor {
    app: Arc<MainApp>,
    literature: Literature,
    type_selector: Entity<SelectState<Vec<LiteratureTypeItem>>>,
    selected_type: LiteratureType,
    title_input: Entity<InputState>,
    authors_input: Entity<InputState>,
    journal_input: Entity<InputState>,
    year_input: Entity<InputState>,
    month_input: Entity<InputState>,
    day_input: Entity<InputState>,
    volume_input: Entity<InputState>,
    issue_input: Entity<InputState>,
    pages_input: Entity<InputState>,
    doi_input: Entity<InputState>,
    arxiv_id_input: Entity<InputState>,
    url_input: Entity<InputState>,
    publisher_input: Entity<InputState>,
    abstract_input: Entity<InputState>,
    notes_input: Entity<InputState>,
    // 回调函数：当完成时调用 (Some(literature) 表示确认修改，None 表示取消)
    on_complete: LiteratureEditorCallback,
}

impl LiteratureEditor {
    pub fn new(
        app: Arc<MainApp>,
        literature: Literature,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_complete: impl Fn(Option<Literature>, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        debug!(
            "EDITOR_NEW: 构造 LiteratureEditor (title='{}')",
            literature.title
        );
        let lang = app.current_language();

        let initial_type = literature.literature_type.clone();
        let types: Vec<_> = <LiteratureType as LiteratureTypeExt>::all()
            .into_iter()
            .map(|lt| LiteratureTypeItem {
                lit_type: lt.clone(),
                title: t(lt.i18n_key(), lang).into(),
            })
            .collect();

        let type_selector = cx.new(|cx| {
            let mut state = SelectState::new(types, None, window, cx);
            state.set_selected_value(&initial_type, window, cx);
            state
        });

        cx.subscribe(
            &type_selector,
            |this: &mut Self, _, event: &SelectEvent<Vec<LiteratureTypeItem>>, _cx| {
                if let SelectEvent::Confirm(Some(lit_type)) = event {
                    this.selected_type = lit_type.clone();
                }
            },
        )
        .detach();

        // 创建所有输入框并填充初始数据
        let title_input = Self::create_input(
            &literature.title,
            t(I18nKey::Title, lang),
            false,
            window,
            cx,
        );

        let authors_str = literature
            .authors
            .iter()
            .map(author_full_name)
            .collect::<Vec<_>>()
            .join(", ");
        let authors_input = Self::create_input(
            &authors_str,
            t(I18nKey::AuthorPlaceholder, lang),
            false,
            window,
            cx,
        );

        let journal_input = Self::create_input(
            &literature
                .publication
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            t(I18nKey::JournalPlaceholder, lang),
            false,
            window,
            cx,
        );
        let year_input = Self::create_input(
            &literature.year.map(|y| y.to_string()).unwrap_or_default(),
            t(I18nKey::Year, lang),
            false,
            window,
            cx,
        );
        let month_input = Self::create_input(
            &literature.month.map(|m| m.to_string()).unwrap_or_default(),
            t(I18nKey::Month, lang),
            false,
            window,
            cx,
        );
        let day_input = Self::create_input(
            &literature.day.map(|d| d.to_string()).unwrap_or_default(),
            t(I18nKey::Day, lang),
            false,
            window,
            cx,
        );
        let volume_input = Self::create_input(
            literature.volume.as_deref().unwrap_or(""),
            t(I18nKey::Volume, lang),
            false,
            window,
            cx,
        );
        let issue_input = Self::create_input(
            literature.issue.as_deref().unwrap_or(""),
            t(I18nKey::Issue, lang),
            false,
            window,
            cx,
        );
        let pages_input = Self::create_input(
            literature.pages.as_deref().unwrap_or(""),
            t(I18nKey::Pages, lang),
            false,
            window,
            cx,
        );
        let doi_input = Self::create_input(
            literature.doi.as_deref().unwrap_or(""),
            "DOI",
            false,
            window,
            cx,
        );
        let arxiv_id_input = Self::create_input(
            literature.arxiv_id.as_deref().unwrap_or(""),
            "ArXiv ID",
            false,
            window,
            cx,
        );
        let url_input = Self::create_input(
            literature.url.as_deref().unwrap_or(""),
            "URL",
            false,
            window,
            cx,
        );
        let publisher_input = Self::create_input(
            literature
                .publication
                .as_ref()
                .and_then(|p| p.publisher.as_deref())
                .unwrap_or(""),
            t(I18nKey::Publisher, lang),
            false,
            window,
            cx,
        );
        let abstract_input = Self::create_input(
            literature.abstract_text.as_deref().unwrap_or(""),
            t(I18nKey::Abstract, lang),
            true,
            window,
            cx,
        );
        let notes_initial = app
            .db
            .list_notes(&literature.id)
            .ok()
            .and_then(|notes| notes.into_iter().next().map(|n| n.content))
            .unwrap_or_default();
        let notes_input =
            Self::create_input(&notes_initial, t(I18nKey::Notes, lang), true, window, cx);

        debug!("EDITOR_NEW: 提交 Self (title='{}')", literature.title);
        Self {
            app,
            literature: literature.clone(),
            type_selector,
            selected_type: literature.literature_type.clone(),
            title_input,
            authors_input,
            journal_input,
            year_input,
            month_input,
            day_input,
            volume_input,
            issue_input,
            pages_input,
            doi_input,
            arxiv_id_input,
            url_input,
            publisher_input,
            abstract_input,
            notes_input,
            on_complete: Box::new(on_complete),
        }
    }

    /// 辅助方法：创建并初始化输入框
    fn create_input(
        initial_value: &str,
        placeholder: &str,
        multi_line: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let initial_value: SharedString = initial_value.to_string().into();
        let placeholder: SharedString = placeholder.to_string().into();
        cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder(placeholder)
                .default_value(initial_value);
            if multi_line {
                state = state.multi_line(true).rows(5);
            }
            state
        })
    }

    fn handle_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        info!("编辑器: 用户点击保存，正在提取表单数据...");
        let mut lit = self.literature.clone();

        // 更新文献类型
        lit.literature_type = self.selected_type.clone();

        lit.title = self.title_input.read(cx).text().to_string();
        // 更新 publication 字段
        let journal_text = self.journal_input.read(cx).text().to_string();
        if journal_text.is_empty() {
            lit.publication = None;
        } else {
            // 保留现有类型，如果没有则默认为 Journal
            let pub_type = lit
                .publication
                .as_ref()
                .map_or(PublicationType::Journal, |p| p.publication_type.clone());
            lit.publication = Some(create_publication(journal_text, pub_type));
        }
        lit.volume = Some(self.volume_input.read(cx).text().to_string());
        lit.issue = Some(self.issue_input.read(cx).text().to_string());
        lit.pages = Some(self.pages_input.read(cx).text().to_string());
        lit.doi = Some(self.doi_input.read(cx).text().to_string());
        lit.arxiv_id = Some(self.arxiv_id_input.read(cx).text().to_string());
        lit.url = Some(self.url_input.read(cx).text().to_string());

        let publisher_text = self.publisher_input.read(cx).text().to_string();
        if !publisher_text.is_empty() {
            if let Some(ref mut pub_data) = lit.publication {
                pub_data.publisher = Some(publisher_text);
            } else {
                let pub_type = if lit.literature_type == models::LiteratureType::Conference {
                    PublicationType::Conference
                } else {
                    PublicationType::Journal
                };
                let mut new_pub = create_publication(String::new(), pub_type);
                new_pub.publisher = Some(publisher_text);
                lit.publication = Some(new_pub);
            }
        } else if let Some(ref mut pub_data) = lit.publication {
            pub_data.publisher = None;
        }

        // 自动规范化 ArXiv 标识符
        sanitize_arxiv_identifiers(&mut lit);

        lit.abstract_text = Some(self.abstract_input.read(cx).text().to_string());

        let year_text = self.year_input.read(cx).text().to_string();
        if let Ok(year) = year_text.parse::<i32>() {
            lit.year = Some(year);
        }

        let month_text = self.month_input.read(cx).text().to_string();
        if let Ok(month) = month_text.parse::<i32>() {
            lit.month = Some(month);
        }

        let day_text = self.day_input.read(cx).text().to_string();
        if let Ok(day) = day_text.parse::<i32>() {
            lit.day = Some(day);
        }

        // 解析作者
        let authors_text = self.authors_input.read(cx).text().to_string();
        let authors = parse_author_list(&authors_text);

        if !authors.is_empty() {
            lit.authors = authors;
        }

        let notes_content = self.notes_input.read(cx).text().to_string();
        if !notes_content.is_empty() {
            let existing = self.app.db.list_notes(&lit.id).unwrap_or_default();
            if let Some(first) = existing.into_iter().next() {
                let _ = self
                    .app
                    .db
                    .update_note(&first.id, None, Some(&notes_content));
            } else {
                if let Ok(new_id) = self.app.db.create_note(&lit.id, "笔记") {
                    let _ = self.app.db.update_note(&new_id, None, Some(&notes_content));
                }
            }
        }

        info!(
            "编辑器: 数据提取完成，标题: '{}', 作者数: {}",
            lit.title,
            lit.authors.len()
        );
        (self.on_complete)(Some(lit), window, cx);
    }

    fn _handle_cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        (self.on_complete)(None, window, cx);
    }
}

impl Render for LiteratureEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        debug!(
            "EDITOR_RENDER: 渲染 LiteratureEditor (title='{}')",
            self.literature.title
        );
        let lang = self.app.current_language();

        div()
            .size_full()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .relative()
            .overflow_hidden()
            // 拖拽层：绝对定位覆盖在顶部，不占布局空间
            .child({
                let drag = div()
                    .id("editor-drag-area")
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(rems(2.2));

                #[cfg(not(windows))]
                let drag = drag.on_double_click(|_, window, _| window.remove_window());

                add_drag_behavior(drag, _window, cx)
            })
            // 标题和按钮行
            .child(
                h_flex()
                    .w_full()
                    .px_6()
                    .justify_between()
                    .items_center()
                    .mb_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .child(t(I18nKey::LiteratureEditor, lang)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("cancel-edit")
                                    .child(Icon::new(IconName::Close).size(rems(0.75)))
                                    .ghost()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this._handle_cancel(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("save-edit")
                                    .child(Icon::new(IconName::Check).size(rems(0.75)))
                                    .primary()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.handle_save(window, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .px_6()
                    .pb_6()
                    .min_h(rems(0.0)) // 关键：允许 flex 子项缩小到 0，从而触发内容溢出滚动
                    .overflow_y_scrollbar() // 启用纵向滚动
                    .pr_4() // 增加右侧间距，防止滚动条遮挡内容
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(Label::new(t(I18nKey::Type, lang)).text_sm())
                                    .child(muted_select(
                                        Select::new(&self.type_selector),
                                        cx.theme(),
                                    )),
                            )
                            // ... (其余部分代码保持逻辑一致)
                            .child(LabeledInput::new(
                                t(I18nKey::Title, lang),
                                &self.title_input,
                            ))
                            .child(LabeledInput::new(
                                t(I18nKey::Authors, lang),
                                &self.authors_input,
                            ))
                            .child(LabeledInput::new(
                                t(I18nKey::Journal, lang),
                                &self.journal_input,
                            ))
                            .child(
                                h_flex()
                                    .gap_4()
                                    .child(
                                        LabeledInput::new(t(I18nKey::Year, lang), &self.year_input)
                                            .width(rems(5.0)),
                                    )
                                    .child(
                                        LabeledInput::new(
                                            t(I18nKey::Month, lang),
                                            &self.month_input,
                                        )
                                        .width(rems(3.125)),
                                    )
                                    .child(
                                        LabeledInput::new(t(I18nKey::Day, lang), &self.day_input)
                                            .width(rems(3.125)),
                                    )
                                    .child(
                                        LabeledInput::new(
                                            t(I18nKey::Volume, lang),
                                            &self.volume_input,
                                        )
                                        .width(rems(3.75)),
                                    )
                                    .child(
                                        LabeledInput::new(
                                            t(I18nKey::Issue, lang),
                                            &self.issue_input,
                                        )
                                        .width(rems(3.75)),
                                    )
                                    .child(div().flex_grow(1.0).child(LabeledInput::new(
                                        t(I18nKey::Pages, lang),
                                        &self.pages_input,
                                    ))),
                            )
                            .child(
                                h_flex()
                                    .gap_4()
                                    .child(
                                        div()
                                            .flex_grow(1.0)
                                            .child(LabeledInput::new("DOI", &self.doi_input)),
                                    )
                                    .child(div().flex_grow(1.0).child(LabeledInput::new(
                                        "ArXiv ID",
                                        &self.arxiv_id_input,
                                    ))),
                            )
                            .child(
                                h_flex().gap_4().child(
                                    div()
                                        .flex_grow(1.0)
                                        .child(LabeledInput::new("URL", &self.url_input)),
                                ),
                            )
                            .child(LabeledInput::new(
                                t(I18nKey::Publisher, lang),
                                &self.publisher_input,
                            ))
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(Label::new(t(I18nKey::Abstract, lang)).text_sm())
                                    .child(muted_input(
                                        Input::new(&self.abstract_input).h(rems(7.5)),
                                        cx.theme(),
                                    )),
                            ),
                    ),
            )
    }
}
