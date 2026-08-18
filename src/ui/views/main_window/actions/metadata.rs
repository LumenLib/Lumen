use crate::ui::components::{MetadataSelector, TagSelector};
use crate::ui::dialogs::{CompareDialog, FieldSelection};
use crate::ui::notification::show_notification;
use gpui::prelude::*;
use gpui::{Pixels, Point, Window, px, size};
use gpui_component::notification::NotificationType;
use i18n::{I18nKey, t};
use log::info;
use models::Literature;
use std::sync::Arc;

impl super::super::MainWindow {
    pub(crate) fn show_literature_compare(
        &mut self,
        original: Arc<Literature>,
        new_lit: Literature,
        cx: &mut Context<Self>,
    ) {
        self.show_literature_compare_with_callback(original, new_lit, cx, |_, _| {});
    }

    pub fn show_literature_compare_with_callback(
        &mut self,
        original: Arc<Literature>,
        new_lit: Literature,
        cx: &mut Context<Self>,
        on_done: impl Fn(&mut Self, &mut Context<Self>) + Send + Sync + 'static,
    ) {
        info!("Metadata Compare Debug - Local Data: {original:?}");
        info!("Metadata Compare Debug - Fetched Data: {new_lit:?}");

        let selection = FieldSelection::compare(&original, &new_lit);

        if !selection.has_any_diff() {
            info!("获取元数据: 结果与本地完全一致，无需合并。");
            let lang = self.app.current_language();
            show_notification(
                NotificationType::Info,
                format!(
                    "{}: {}",
                    t(I18nKey::DataConsistentTitle, lang),
                    t(I18nKey::DataConsistentMsg, lang)
                ),
                cx,
            );
            on_done(self, cx);
            return;
        }

        let app = self.app.clone();
        let size = size(px(1100.0), px(800.0));
        let this_weak = cx.entity().downgrade();
        let on_done = Arc::new(on_done);

        self.open_modal_window(size, cx, move |_window, _cx| {
            let on_done_cb = on_done.clone();
            let this_weak_cb = this_weak.clone();

            CompareDialog::new_with_data(app, original, new_lit, selection, move |_, window, cx| {
                window.remove_window();
                if let Some(this) = this_weak_cb.upgrade() {
                    this.update(cx, |this, cx| {
                        on_done_cb(this, cx);
                    });
                }
            })
        });
    }

    pub fn open_tag_selector(
        &mut self,
        current_tags: Vec<String>,
        on_select: impl Fn(String, &mut Window, &mut Context<TagSelector>) + Send + Sync + 'static,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();

        let selector = TagSelector::build(
            app,
            current_tags,
            window,
            cx,
            on_select,
            move |_window, cx| {
                let _ = this_weak.update(cx, |this, cx| {
                    this.tag_selector = None;
                    cx.notify();
                });
            },
        );
        self.tag_selector = Some((selector, position));
        cx.notify();
    }

    pub fn open_metadata_selector(
        &mut self,
        candidates: Vec<Arc<Literature>>,
        cx: &mut Context<Self>,
        on_select: impl Fn(&mut Self, Literature, &mut Window, &mut Context<Self>)
        + Send
        + Sync
        + 'static,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let on_select = Arc::new(on_select);
        let size = size(px(660.0), px(580.0));

        self.open_modal_window(size, cx, move |_window, _cx| {
            MetadataSelector::new(app, candidates, move |result, window, cx| {
                if let Some(lit) = result
                    && let Some(this) = this_weak.upgrade()
                {
                    let on_select = on_select.clone();
                    this.update(cx, |this, cx| {
                        on_select(this, lit, window, cx);
                    });
                }
                window.remove_window();
            })
        });
    }

    pub fn open_citation_selector(
        &mut self,
        exclude_id: String,
        on_select: impl Fn(String, &mut Window, &mut Context<Self>) + Send + Sync + 'static,
        cx: &mut Context<Self>,
    ) {
        let on_select = Arc::new(on_select);

        let candidates = {
            let data = self.data_store.read(cx);
            data.literatures
                .iter()
                .filter(|lit| lit.id != exclude_id)
                .cloned()
                .collect::<Vec<_>>()
        };

        self.open_metadata_selector(candidates, cx, move |_, lit: Literature, window, cx| {
            let on_select = on_select.clone();
            on_select(lit.id, window, cx);
        });
    }
}
