use crate::RUNTIME;
use crate::services::MainApp;
use crate::ui::components::muted_input;
use gpui::prelude::*;
use gpui::{
    AnyWindowHandle, App, AppContext, AsyncApp, Entity, FontWeight, SharedString, Window, div,
};
use gpui_component::{
    ActiveTheme,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use i18n::{I18nKey, Language, t, tf};
use log::{debug, error, info};
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
enum FetchState {
    Input,
    Fetching(SharedString),
    Error(SharedString),
}

type OnComplete = Box<dyn FnOnce(Vec<Literature>, &mut Window, &mut App) + 'static>;

pub struct FetchDialogContent {
    app: Arc<MainApp>,
    mode: FetchMode,
    state: FetchState,
    input: Entity<InputState>,
    on_complete: Option<OnComplete>,
    main_window_handle: AnyWindowHandle,
}

impl FetchDialogContent {
    pub fn new(
        app: Arc<MainApp>,
        mode: FetchMode,
        main_window_handle: AnyWindowHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Self {
        let lang = app.current_language();
        let placeholder = match mode {
            FetchMode::Doi => t(I18nKey::FetchPlaceholderDoi, lang),
            FetchMode::ArXiv => t(I18nKey::FetchPlaceholderArxiv, lang),
            FetchMode::BibTeX => t(I18nKey::FetchPlaceholderBibtex, lang),
            FetchMode::Dblp => t(I18nKey::FetchPlaceholderDblp, lang),
            FetchMode::OpenAlex => t(I18nKey::FetchPlaceholderOpenAlex, lang),
        };

        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state = state.placeholder(placeholder);
            state
        });

        // 自动聚焦输入框
        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        Self {
            app,
            mode,
            state: FetchState::Input,
            input,
            on_complete: None,
            main_window_handle,
        }
    }

    pub fn set_on_complete(&mut self, cb: OnComplete) {
        self.on_complete = Some(cb);
    }

    pub fn input_entity(&self) -> &Entity<InputState> {
        &self.input
    }

    pub fn mode(&self) -> FetchMode {
        self.mode
    }

    pub fn handle_fetch(&mut self, cx: &mut Context<Self>) {
        let lang = self.app.current_language();
        let id = self.input.read(cx).text().to_string();
        if id.trim().is_empty() {
            return;
        }

        self.state = FetchState::Fetching(id.clone().into());
        cx.notify();

        let app = self.app.clone();
        let mode = self.mode;
        let id_clone = id.clone();
        let window_handle = self.main_window_handle;
        let this_weak = cx.entity().downgrade();

        cx.spawn(move |_, cx: &mut AsyncApp| {
            let mut cx = cx.clone();
            async move {
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
                    let _ = this_weak.update(cx, |this, cx| match result {
                        Ok(lits) => {
                            info!("抓取成功: {} 条记录", lits.len());
                            debug!(
                                "FETCH_DEBUG: on_complete 即将触发, lits.len={}, mode={:?}",
                                lits.len(),
                                this.mode
                            );
                            if let Some(cb) = this.on_complete.take() {
                                cb(lits, window, cx);
                            }
                        }
                        Err(e) => {
                            error!("抓取失败: {e}");
                            this.state = FetchState::Error(e.to_string().into());
                            cx.notify();
                        }
                    });
                });
            }
        })
        .detach();
    }

    fn render_input(&self, lang: Language, _cx: &mut Context<Self>) -> impl IntoElement {
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
                            .text_color(_cx.theme().foreground)
                            .child(tf(I18nKey::FetchFromSource, lang, &[mode_text])),
                    )
                    .child(
                        h_flex().gap_2().child(
                            Button::new("fetch-btn")
                                .child(t(I18nKey::ConfirmFetch, lang))
                                .primary()
                                .on_click(_cx.listener(|this, _, _, cx| {
                                    this.handle_fetch(cx);
                                })),
                        ),
                    ),
            )
            .child(muted_input(Input::new(&self.input), _cx.theme()))
    }

    fn render_fetching(
        &self,
        id: &str,
        lang: Language,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .items_center()
            .justify_center()
            .gap_4()
            .size_full()
            .child({
                let label = match self.mode {
                    FetchMode::BibTeX => "BibTeX".into(),
                    _ => id.to_string(),
                };
                div().child(format!("{}: {}", t(I18nKey::LoadingMetadata, lang), label))
            })
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
                            .text_color(cx.theme().danger)
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

impl Render for FetchDialogContent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = self.app.current_language();
        match &self.state {
            FetchState::Input => self.render_input(lang, cx).into_any_element(),
            FetchState::Fetching(id) => self.render_fetching(id, lang, cx).into_any_element(),
            FetchState::Error(e) => self.render_error(e, lang, cx).into_any_element(),
        }
    }
}
