use crate::ui::dialogs::SubscriptionDialog;
use gpui::prelude::*;
use gpui::{AppContext, Window, px};
use gpui_component::{ActiveTheme, WindowExt, dialog::DialogButtonProps};
use i18n::{I18nKey, t};
use log::error;
use models::Feed;

impl super::super::MainWindow {
    pub fn open_edit_subscription_modal(
        &mut self,
        feed_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let feed = {
            let data = self.data_store.read(cx);
            data.feeds.iter().find(|f| f.id == feed_id).cloned()
        };

        if let Some(feed) = feed {
            self.open_subscription_dialog(Some((*feed).clone()), window, cx);
        }
    }

    pub fn open_add_subscription_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_subscription_dialog(None, window, cx);
    }

    /// 以应用内 dialog 弹窗（而非独立 OS 窗口）打开添加 / 编辑订阅界面。
    fn open_subscription_dialog(
        &mut self,
        feed: Option<Feed>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let this_weak = cx.entity().downgrade();
        let is_edit = feed.is_some();

        // 1. 创建 SubscriptionDialog 实体，仅承载表单状态
        let entity = cx.new(|cx| SubscriptionDialog::new(app.clone(), window, cx, feed));
        self.subscription_dialog = Some(entity.clone());

        let lang = app.current_language();
        let title = if is_edit {
            t(I18nKey::EditSubscription, lang)
        } else {
            t(I18nKey::AddSubscription, lang)
        };

        // 2. 打开应用内 dialog 弹窗（渲染进 dialog 层）
        window.open_dialog(cx, move |dialog, _window, _cx| {
            let theme = _cx.theme();
            dialog
                .w(px(420.0))
                .bg(theme.muted)
                .title(title)
                .content({
                    let this_weak = this_weak.clone();
                    move |content, _, cx| {
                        let entity = this_weak
                            .upgrade()
                            .and_then(|this| this.read(cx).subscription_dialog.clone());
                        if let Some(entity) = entity {
                            content.child(entity.clone())
                        } else {
                            content
                        }
                    }
                })
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(true)
                        .ok_text(if is_edit {
                            t(I18nKey::Save, lang)
                        } else {
                            t(I18nKey::Add, lang)
                        })
                        .cancel_text(t(I18nKey::Cancel, lang))
                        .on_ok({
                            let this_weak = this_weak.clone();
                            move |_, _window, cx| {
                                let Some(this) = this_weak.upgrade() else {
                                    return true;
                                };
                                // 读取输入框内容（不可变借用，结束后立即释放）
                                let values =
                                    this.read(cx).subscription_dialog.as_ref().map(|entity| {
                                        let e = entity.read(cx);
                                        (
                                            e.name_input.read(cx).text().to_string(),
                                            e.url_input.read(cx).text().to_string(),
                                            e.interval_input
                                                .read(cx)
                                                .text()
                                                .to_string()
                                                .parse::<u32>()
                                                .unwrap_or(24),
                                            e.feed_id.clone(),
                                            e.is_edit,
                                        )
                                    });
                                let Some((name, url, interval, feed_id, is_edit)) = values else {
                                    return true;
                                };
                                if name.is_empty() || url.is_empty() {
                                    return false; // 校验失败，保持弹窗打开
                                }
                                this.update(cx, |this, cx| {
                                    let res = if let Some(fid) = feed_id {
                                        this.app.clone().update_feed(fid, name, url, interval)
                                    } else {
                                        this.app.clone().add_feed(name, url, interval)
                                    };
                                    if let Err(e) = res {
                                        error!(
                                            "{}订阅失败: {}",
                                            if is_edit { "更新" } else { "添加" },
                                            e
                                        );
                                    }
                                    this.subscription_dialog = None;
                                    cx.notify();
                                });
                                true
                            }
                        })
                        .on_cancel({
                            let this_weak = this_weak.clone();
                            move |_, _, cx| {
                                if let Some(this) = this_weak.upgrade() {
                                    this.update(cx, |this, cx| {
                                        this.subscription_dialog = None;
                                        cx.notify();
                                    });
                                }
                                true
                            }
                        }),
                )
        });

        // 3. 自动聚焦名称输入框
        window.defer(cx, {
            let entity = entity.clone();
            move |window, cx| {
                entity.update(cx, |this, cx| {
                    this.name_input.update(cx, |state, cx| {
                        state.focus(window, cx);
                    });
                });
            }
        });
        cx.notify();
    }
}
