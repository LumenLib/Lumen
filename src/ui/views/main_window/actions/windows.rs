use crate::ui::views::settings::{SettingsTab, SettingsWindow};
use gpui::prelude::*;
use gpui::{
    AppContext, Bounds, Pixels, Size, Window, WindowBounds, WindowKind, WindowOptions, px, size,
};
use gpui_component::Root;
use log::{debug, error, info};

impl super::super::MainWindow {
    pub fn open_settings_modal(&mut self, cx: &mut Context<Self>, target_tab: Option<SettingsTab>) {
        info!("UI: 用户打开设置对话框, 目标标签: {target_tab:?}");
        let app = self.app.clone();
        let size = size(px(850.0), px(600.0));

        self.open_modal_window(size, cx, move |window, cx| {
            SettingsWindow::new(app, window, cx, target_tab)
        });
    }

    pub(crate) fn open_modal_window<V: Render>(
        &mut self,
        size: Size<Pixels>,
        cx: &mut Context<Self>,
        build_view: impl FnOnce(&mut Window, &mut Context<V>) -> V + Send + 'static,
    ) {
        debug!(
            "MODAL_DEBUG: open_modal_window 入口, active_popup_count={}, size={:?}",
            self.active_popup_count, size,
        );
        if self.active_popup_count > 0 {
            debug!("MODAL: 已有活跃弹窗，跳过 (size={:?})", size);
            return;
        }
        let bounds = Bounds::centered(None, size, cx);
        debug!("MODAL: 开始创建窗口 (size={:?}, bounds={:?})", size, bounds);

        self.active_popup_count += 1;
        debug!(
            "MODAL_DEBUG: active_popup_count 增至 {}",
            self.active_popup_count
        );
        cx.notify();

        let this_weak = cx.entity().downgrade();
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                is_resizable: false,
                is_minimizable: false,
                app_owns_titlebar_drag: true,
                kind: WindowKind::Floating,
                ..Default::default()
            },
            move |window, cx| {
                debug!("MODAL: open_window 内部回调执行 (build_view)");
                let view = cx.new(|cx| build_view(window, cx));
                let root = cx.new(|cx| Root::new(view, window, cx));

                cx.observe_release(&root, move |_, cx| {
                    debug!("MODAL: 窗口根组件已释放");
                    if let Some(this) = this_weak.upgrade() {
                        this.update(cx, |this, cx| {
                            this.active_popup_count = this.active_popup_count.saturating_sub(1);
                            debug!(
                                "MODAL_DEBUG: active_popup_count 降至 {} (after release)",
                                this.active_popup_count
                            );
                            if this.active_popup_count == 0 {
                                if !this.pending_selectors.is_empty() {
                                    this.process_next_pending_selector(cx);
                                } else {
                                    this.process_next_pending_import(cx);
                                }
                            }
                            cx.notify();
                        });
                    }
                })
                .detach();
                // Windows 下程序化打开的窗口默认不会获得前台焦点，会落到主窗口后面，
                // 这里显式将其激活到前台。defer 确保窗口已创建并显示后再激活。
                window.defer(cx, |window, _cx| {
                    window.activate_window();
                });
                root
            },
        );

        if let Err(e) = result {
            error!("MODAL: 窗口创建失败分支 (重复): {e}");
            self.active_popup_count = self.active_popup_count.saturating_sub(1);
            cx.notify();
        }
    }

    pub fn handle_empty_trash(&mut self, cx: &mut Context<Self>) {
        info!("UI: handle_empty_trash triggered");
        let app = self.app.clone();
        cx.spawn(move |_, _cx: &mut gpui::AsyncApp| async move {
            info!("Async Task: Starting empty_trash logic");
            if let Err(e) = app.empty_trash() {
                error!("清空回收站失败: {e}");
            } else {
                info!("Async Task: empty_trash completed successfully");
            }
        })
        .detach();
    }
}
