use crate::RUNTIME;
use crate::services::MainApp;
use crate::ui::icons::IconName;
use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, AppContext, AsyncApp, Entity, FontWeight, WeakEntity, Window,
    WindowControlArea, div, red, rems,
};
use gpui_component::input::InputEvent;
use gpui_component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use i18n::{I18nKey, Language, t, tf};
use log::{error, info};
use models::Literature;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FetchMode {
    Doi,
    ArXiv,
    BibTeX,
    Dblp,
    OpenAlex,
}

#[derive(Clone)]
pub enum FetchState {
    Input,
    Fetching(String),
    Error(String),
}

pub type LiteratureFetcherCallback = Box<
    dyn Fn(Option<Vec<Literature>>, &mut Window, &mut Context<LiteratureFetcher>) + Send + Sync,
>;

/// 文献抓取网关 (处理 DOI/ArXiv/BibTeX/DBLP 的输入与解析)
pub struct LiteratureFetcher {
    app: Arc<MainApp>,
    mode: FetchMode,
    state: FetchState,
    input: Entity<InputState>,
    window_handle: AnyWindowHandle,
    // 回调函数：当完成时调用 (Some(literatures) 表示抓取成功，None 表示取消)
    on_complete: LiteratureFetcherCallback,
}

impl LiteratureFetcher {
    pub fn new(
        app: Arc<MainApp>,
        mode: FetchMode,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_complete: impl Fn(Option<Vec<Literature>>, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        let lang = app.current_language();
        let placeholder = match mode {
            FetchMode::Doi => t(I18nKey::FetchPlaceholderDoi, lang),
            FetchMode::ArXiv => t(I18nKey::FetchPlaceholderArxiv, lang),
            FetchMode::BibTeX => t(I18nKey::FetchPlaceholderBibtex, lang),
            FetchMode::Dblp => t(I18nKey::FetchPlaceholderDblp, lang),
            FetchMode::OpenAlex => t(I18nKey::FetchPlaceholderOpenAlex, lang),
        };

        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));

        // ... (其余代码)

        // 自动聚焦输入框
        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        cx.subscribe(&input, move |this, _, event, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.handle_fetch(cx);
            }
        })
        .detach();

        Self {
            app,
            mode,
            state: FetchState::Input,
            input,
            window_handle: window.window_handle(),
            on_complete: Box::new(on_complete),
        }
    }

    fn handle_fetch(&mut self, cx: &mut Context<Self>) {
        let lang = self.app.current_language();
        let id = self.input.read(cx).text().to_string();
        if id.trim().is_empty() {
            return;
        }

        self.state = FetchState::Fetching(id.clone());
        cx.notify();

        let app = self.app.clone();
        let mode = self.mode;
        let id_clone = id.clone();
        let window_handle = self.window_handle;

        cx.spawn(move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
                info!("开始抓取文献: {id_clone} (模式: {mode:?})");

                let result_handle = RUNTIME.spawn(async move {
                    match mode {
                        FetchMode::Doi => app
                            .fetcher_service
                            .parse_doi(&id_clone)
                            .await
                            .map(|lit| vec![lit]),
                        FetchMode::ArXiv => app
                            .fetcher_service
                            .parse_arxiv(&id_clone)
                            .await
                            .map(|lit| vec![lit]),
                        FetchMode::BibTeX => {
                            let results = app.fetcher_service.parse_bibtex(&id_clone)?;
                            if results.is_empty() {
                                Err(anyhow::anyhow!(t(I18nKey::NoContentOrInvalidFormat, lang)))
                            } else {
                                Ok(results)
                            }
                        }
                        FetchMode::Dblp => {
                            let results = app.fetcher_service.search_dblp(&id_clone).await?;
                            if results.is_empty() {
                                Err(anyhow::anyhow!(t(I18nKey::NoMatchFound, lang)))
                            } else {
                                Ok(results)
                            }
                        }
                        FetchMode::OpenAlex => {
                            let results =
                                app.fetcher_service.search_openalex(&id_clone, 10).await?;
                            if results.is_empty() {
                                Err(anyhow::anyhow!(t(I18nKey::NoMatchFound, lang)))
                            } else {
                                Ok(results)
                            }
                        }
                    }
                });

                let result = match result_handle.await {
                    Ok(res) => res,
                    Err(e) => Err(anyhow::anyhow!(format!(
                        "{}: {}",
                        t(I18nKey::FetchFailed, lang),
                        e
                    ))),
                };

                let _ = cx.update_window(window_handle, |_, window, cx| {
                    let _ = this.update(cx, |this, cx| match result {
                        Ok(lits) => {
                            info!("抓取成功: {} 条记录", lits.len());
                            (this.on_complete)(Some(lits), window, cx);
                        }
                        Err(e) => {
                            error!("抓取失败: {e}");
                            this.state = FetchState::Error(e.to_string());
                            cx.notify();
                        }
                    });
                });
            }
        })
        .detach();
    }

    fn render_input(&self, lang: Language, cx: &mut Context<Self>) -> impl IntoElement {
        let mode_text = match self.mode {
            FetchMode::Doi => "DOI",
            FetchMode::ArXiv => "ArXiv",
            FetchMode::BibTeX => "BibTeX",
            FetchMode::Dblp => "DBLP",
            FetchMode::OpenAlex => "OpenAlex",
        };

        v_flex()
            .gap_3()
            .size_full()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child(tf(I18nKey::FetchFromSource, lang, &[mode_text])),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("fetch-btn")
                                .child(t(I18nKey::ConfirmFetch, lang))
                                .primary()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.handle_fetch(cx);
                                })),
                        ),
                    ),
            )
            .child(Input::new(&self.input))
    }

    fn render_fetching(
        &self,
        id: &str,
        lang: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .items_center()
            .justify_center() // 居中显示加载状态
            .gap_4()
            .size_full()
            .child(div().child(format!("{}: {}", t(I18nKey::LoadingMetadata, lang), id)))
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .child(t(I18nKey::LoadingMetadata, lang)),
            )
    }

    fn render_error(&self, err: &str, lang: Language, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .size_full()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(red())
                            .child(t(I18nKey::FetchFailed, lang)),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("retry-btn")
                                .child(t(I18nKey::Retry, lang))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.state = FetchState::Input;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .child(div().child(err.to_string()))
    }
}

impl Render for LiteratureFetcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self.app.current_language();
        div()
            .size_full()
            .bg(cx.theme().background)
            .px_6()
            .pt(rems(2.0))
            .pb_4()
            .when(cfg!(not(target_os = "macos")), |this: gpui::Div| {
                this.child(
                    div()
                        .h(rems(2.0))
                        .w_full()
                        .absolute()
                        .top_0()
                        .left_0()
                        .window_control_area(WindowControlArea::Drag),
                )
                // Window controls
                .child(
                    div()
                        .absolute()
                        .top_1()
                        .right_1()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .id("fetch-modal-close-btn")
                                .h(rems(1.5))
                                .w(rems(1.5))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_sm()
                                .cursor_pointer()
                                .occlude()
                                .window_control_area(WindowControlArea::Close)
                                .hover(|s| s.bg(gpui::red().opacity(0.9)))
                                .child(
                                    Icon::new(IconName::Close)
                                        .size(rems(0.875))
                                        .text_color(cx.theme().foreground),
                                ),
                        ),
                )
            })
            .child(match &self.state {
                FetchState::Input => self.render_input(lang, cx).into_any_element(),
                FetchState::Fetching(id) => self.render_fetching(id, lang, cx).into_any_element(),
                FetchState::Error(e) => self.render_error(e, lang, cx).into_any_element(),
            })
    }
}
