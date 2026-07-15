use crate::ui::theme_manager::surface;
use gpui::prelude::*;
use gpui::{
    App, Context, Entity, FocusHandle, Focusable, MouseButton, ScrollHandle, SharedString, Window,
    div, px, rems,
};
use gpui_component::{ActiveTheme, TitleBar, h_flex};
use log::info;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config_store::ConfigStore;
use crate::services::MainApp;
use crate::ui::views::main_window::actions::AppPdfDelegate;
use models::Literature;
use pdf::PdfReaderView;

pub struct PdfWindowController {
    app: Arc<MainApp>,
    active_tab_id: Option<String>,
    open_pdf_tabs: HashMap<String, Option<Entity<PdfReaderView>>>,
    open_pdf_tab_order: Vec<String>,
    pdf_tab_titles: HashMap<String, String>,
    pdf_tab_paths: HashMap<String, (Arc<Literature>, Option<PathBuf>)>,
    tab_scroll_handle: ScrollHandle,
}

impl PdfWindowController {
    pub fn new(app: Arc<MainApp>, cx: &mut Context<Self>) -> Self {
        // 监听全局主题和配置更新，使独立窗口能实时同步重绘
        cx.observe_global::<gpui_component::Theme>(|_, cx| {
            cx.notify();
        })
        .detach();

        cx.observe_global::<ConfigStore>(|_, cx| {
            cx.notify();
        })
        .detach();

        Self {
            app,
            active_tab_id: None,
            open_pdf_tabs: HashMap::new(),
            open_pdf_tab_order: Vec::new(),
            pdf_tab_titles: HashMap::new(),
            pdf_tab_paths: HashMap::new(),
            tab_scroll_handle: ScrollHandle::new(),
        }
    }

    pub fn open_pdf(&mut self, lit: Arc<Literature>, path: PathBuf, cx: &mut Context<Self>) {
        let doc_id = lit
            .attachments
            .iter()
            .find(|a| a.file_path == path.to_string_lossy())
            .map(|a| format!("{}::{}", lit.id, a.id))
            .unwrap_or_else(|| lit.id.clone());

        if self.open_pdf_tabs.contains_key(&doc_id) {
            info!("PdfWindowController: PDF 已打开，切换标签: {doc_id}");
            self.activate_pdf_tab(doc_id, cx);
            return;
        }

        self.pdf_tab_titles
            .insert(doc_id.clone(), lit.title.clone());
        self.pdf_tab_paths
            .insert(doc_id.clone(), (lit.clone(), Some(path)));

        self.open_pdf_tabs.insert(doc_id.clone(), None);
        self.open_pdf_tab_order.push(doc_id.clone());

        self.activate_pdf_tab(doc_id, cx);
    }

    pub fn activate_pdf_tab(&mut self, doc_id: String, cx: &mut Context<Self>) {
        self.active_tab_id = Some(doc_id.clone());

        if self.open_pdf_tabs.get(&doc_id).is_none_or(|v| v.is_none()) {
            self.reload_pdf_tab(doc_id, cx);
        }
        cx.notify();
    }

    fn reload_pdf_tab(&mut self, doc_id: String, cx: &mut Context<Self>) {
        if let Some((lit, preferred_path)) = self.pdf_tab_paths.get(&doc_id).cloned() {
            let path = preferred_path.clone().or_else(|| {
                lit.attachments
                    .iter()
                    .find(|a| a.is_main)
                    .map(|a| PathBuf::from(&a.file_path))
            });
            let Some(path) = path else {
                return;
            };

            let app = self.app.clone();
            let doc_id_for_open = doc_id.clone();
            let lit_id = lit.id.clone();

            let (pdf_service, response_rx) =
                pdf::PdfService::new(path.clone()).expect("Failed to create PdfService");
            let delegate = Arc::new(AppPdfDelegate {
                app: app.clone(),
                literature_id: lit_id,
            });

            let view = cx.new(|cx| {
                let mut view = PdfReaderView::new(pdf_service, Some(delegate), doc_id_for_open, cx);
                // 顶部为手写 TabBar，为防止遮挡，增加 35px 偏置
                view.set_tab_bar_offset_px(35.0);
                view.set_document_title(lit.title.clone());
                view.init_workers(response_rx, cx);
                view
            });

            // 观察全局配置更新（如语言等）
            cx.observe_global::<ConfigStore>({
                let view_weak = view.downgrade();
                move |_this: &mut Self, cx: &mut gpui::Context<Self>| {
                    if let Some(view) = view_weak.upgrade() {
                        view.update(cx, |this, cx| {
                            let lang = cx.global::<ConfigStore>().current_language();
                            this.set_language(lang, cx);
                        });
                    }
                }
            })
            .detach();

            self.open_pdf_tabs.insert(doc_id, Some(view));
        }
    }

    pub fn close_pdf_tab(&mut self, doc_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_pdf_tabs.remove(doc_id);
        self.pdf_tab_titles.remove(doc_id);
        self.pdf_tab_paths.remove(doc_id);
        self.open_pdf_tab_order.retain(|id| id != doc_id);

        if Some(doc_id.to_string()) == self.active_tab_id {
            self.active_tab_id = self.open_pdf_tab_order.last().cloned();
            if let Some(ref active_id) = self.active_tab_id {
                self.activate_pdf_tab(active_id.clone(), cx);
            }
        }

        // 如果全部 Tab 已关闭，自动销毁当前独立 PDF 窗口
        if self.open_pdf_tab_order.is_empty() {
            info!("PdfWindowController: 所有 PDF 标签已关闭，销毁 PDF 窗口");
            window.remove_window();
        } else {
            cx.notify();
        }
    }

    pub fn reload_all_pdf_tabs(&mut self, cx: &mut Context<Self>) {
        for view in self.open_pdf_tabs.values().flatten() {
            view.update(cx, |v, cx| {
                v.reload_notes(cx);
                v.reload_chat_sessions(cx);
            });
        }
    }

    fn active_pdf_view(&self) -> Option<Entity<PdfReaderView>> {
        self.active_tab_id
            .as_ref()
            .and_then(|id| self.open_pdf_tabs.get(id).cloned().flatten())
    }

    fn render_tab_bar(&self, _window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        TitleBar::new()
            .bg(theme.title_bar)
            .border_color(theme.title_bar_border)
            .child(
                h_flex()
                    .id("pdf-bar")
                    .h_full()
                    .w_full()
                    .items_center()
                    // 可滚动标签区
                    .child(
                        div()
                            .id("pdf-tab-scroll-area")
                            .h_full()
                            .flex()
                            .flex_row()
                            .flex_grow(1.0)
                            .min_w(px(0.0))
                            .overflow_x_scroll()
                            .track_scroll(&self.tab_scroll_handle)
                            .items_center()
                            .children(self.open_pdf_tab_order.iter().map(|doc_id| {
                                let is_active = Some(doc_id.to_string()) == self.active_tab_id;
                                let title = self
                                    .pdf_tab_titles
                                    .get(doc_id)
                                    .map(|s| s.as_str())
                                    .unwrap_or(doc_id);
                                let tab_id: SharedString = format!("tab-pdf-{doc_id}").into();
                                let doc_id_for_click = doc_id.clone();
                                let doc_id_for_close = doc_id.clone();

                                div()
                                    .id(tab_id)
                                    .px(rems(0.75))
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .when(is_active, |this| {
                                        this.bg(theme.secondary_active).text_color(theme.foreground)
                                    })
                                    .when(!is_active, |this| {
                                        this.hover(|this| this.bg(theme.secondary_hover))
                                            .text_color(theme.foreground)
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            this.activate_pdf_tab(doc_id_for_click.clone(), cx);
                                        }),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .items_center()
                                            .child(
                                                div()
                                                    .max_w(rems(12.0))
                                                    .truncate()
                                                    .text_size(rems(0.75))
                                                    .child(title.to_string()),
                                            )
                                            .child(
                                                div()
                                                    .id(format!("close-{}", doc_id_for_close))
                                                    .cursor_pointer()
                                                    .rounded_sm()
                                                    .hover(|this| this.bg(surface().danger_ghost))
                                                    .px(rems(0.25))
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |this, _event, window, cx| {
                                                                cx.stop_propagation();
                                                                this.close_pdf_tab(
                                                                    &doc_id_for_close,
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        ),
                                                    )
                                                    .text_size(rems(0.75))
                                                    .child("✕"),
                                            ),
                                    )
                                    .into_any_element()
                            })),
                    )
                    .child(div().flex_grow(1.0)),
            )
    }
}

impl Render for PdfWindowController {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .child(self.render_tab_bar(window, cx))
            .child(div().h(px(1.0)).w_full().flex_none().bg(cx.theme().border))
            .child(
                div()
                    .flex_grow(1.0)
                    .child(if let Some(view) = self.active_pdf_view() {
                        view.into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }
}

impl Focusable for PdfWindowController {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if let Some(view) = self.active_pdf_view() {
            view.focus_handle(cx)
        } else {
            cx.focus_handle()
        }
    }
}
