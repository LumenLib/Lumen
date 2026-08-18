use crate::ui::notification::show_notification;
use crate::ui::views::PdfWindowController;
use gpui::prelude::*;
use gpui::{AppContext, px, size};
use gpui_component::{Root, TitleBar, notification::NotificationType};
use i18n::{I18nKey, t, tf};
use log::{error, info};
use models::Literature;
use std::path::PathBuf;
use std::sync::Arc;

impl super::super::MainWindow {
    pub fn open_pdf_viewer(&mut self, lit: Arc<Literature>, cx: &mut Context<Self>) {
        self.open_pdf_viewer_with_path(lit, None, cx);
    }

    pub fn open_pdf_viewer_with_path(
        &mut self,
        lit: Arc<Literature>,
        preferred_path: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = preferred_path.or_else(|| {
            lit.attachments
                .iter()
                .find(|a| a.is_main)
                .map(|a| PathBuf::from(&a.file_path))
                .or_else(|| {
                    lit.attachments
                        .iter()
                        .find(|a| a.file_path.to_lowercase().ends_with(".pdf"))
                        .map(|a| PathBuf::from(&a.file_path))
                })
        }) else {
            error!("MainWindow: 该文献没有 PDF 附件");
            return;
        };

        info!("MainWindow: 尝试打开 PDF 阅读器, 路径: {:?}", path);
        if !path.exists() {
            error!("MainWindow: PDF 文件不存在: {:?}", path);
            let lang = self.app.current_language();
            show_notification(
                NotificationType::Error,
                format!(
                    "{}: {}",
                    t(I18nKey::FileNotFoundTitle, lang),
                    tf(I18nKey::FileNotFoundMsg, lang, &[&format!("{:?}", path)])
                ),
                cx,
            );
            return;
        }

        let doc_id = lit
            .attachments
            .iter()
            .find(|a| a.file_path == path.to_string_lossy())
            .map(|a| format!("{}::{}", lit.id, a.id))
            .unwrap_or_else(|| lit.id.clone());

        let this_weak = cx.entity().downgrade();

        // 尝试升级已有的 PDF 窗口控制器并置顶
        if let Some(ref weak_ctrl) = self.pdf_window_controller
            && let Some(controller) = weak_ctrl.upgrade()
            && let Some(ref handle) = self.pdf_window_handle
        {
            info!("MainWindow: 独立 PDF 窗口已处于开启状态，添加并激活 PDF 标签: {doc_id}");
            controller.update(cx, |this, cx| {
                this.open_pdf(lit.clone(), path.clone(), cx);
            });
            let _ = handle.update(cx, |_, window, _| {
                window.activate_window();
            });
            return;
        }

        // 否则，开启全新独立的 PDF 窗口
        info!("MainWindow: 开启全新独立的 PDF 窗口以阅读 PDF: {doc_id}");
        let app = self.app.clone();

        let lit_for_cb = lit.clone();
        let path_for_cb = path.clone();
        let controller_weak = Arc::new(std::sync::Mutex::new(None));
        let controller_weak_cb = controller_weak.clone();

        let screen_size = cx
            .displays()
            .first()
            .map(|d| d.bounds().size)
            .unwrap_or_else(|| gpui::size(px(1920.0), px(1080.0)));
        let initial_size = gpui::size(
            px(screen_size.width.as_f32() * 0.9),
            px(screen_size.height.as_f32() * 0.9),
        );
        let window_bounds = Some(gpui::WindowBounds::Maximized(gpui::Bounds::centered(
            None,
            initial_size,
            cx,
        )));

        let self_handle = self.self_handle;
        let result = cx.open_window(
            gpui::WindowOptions {
                window_bounds,
                titlebar: Some(TitleBar::title_bar_options()),
                app_owns_titlebar_drag: true,
                is_resizable: true,
                is_minimizable: true,
                kind: gpui::WindowKind::Normal,
                window_min_size: Some(size(px(800.0), px(500.0))),
                ..Default::default()
            },
            move |window, cx| {
                let controller = cx.new(|cx| PdfWindowController::new(app, Some(self_handle), cx));
                *controller_weak_cb.lock().unwrap() = Some(controller.downgrade());

                controller.update(cx, |this, cx| {
                    this.open_pdf(lit_for_cb, path_for_cb, cx);
                });

                // 监听独立窗口即将关闭的事件，以在此刻物理释放所有 GPU 纹理
                let controller_for_close = controller.clone();
                window.on_window_should_close(cx, move |window, cx| {
                    info!("MainWindow: 独立 PDF 窗口即将关闭，执行全量 GPU 纹理物理释放...");
                    let images_to_drop =
                        controller_for_close.update(cx, |this, cx| this.drain_all_tab_images(cx));
                    let count = images_to_drop.len();
                    for img in images_to_drop {
                        if let Err(e) = window.drop_image(img) {
                            log::error!("drop_image failed: {e}");
                        }
                    }
                    info!(
                        "MainWindow: 独立 PDF 窗口即将关闭，物理释放 {} 个纹理完成",
                        count
                    );
                    true // 允许窗口正常关闭
                });

                let root = cx.new(|cx| Root::new(controller.clone(), window, cx));

                // 监听窗口释放以清理句柄
                let this_weak_for_release = this_weak.clone();
                cx.observe_release(&root, move |_, cx| {
                    if let Some(this) = this_weak_for_release.upgrade() {
                        this.update(cx, |this, cx| {
                            info!("MainWindow: 监测到独立 PDF 窗口已释放，清空关联句柄");
                            this.pdf_window_handle = None;
                            this.pdf_window_controller = None;
                            cx.notify();
                        });
                    }
                })
                .detach();

                // Windows/macOS 下显式前台激活
                window.defer(cx, |window, _| {
                    window.activate_window();
                });

                root
            },
        );

        match result {
            Ok(handle) => {
                let weak_controller = controller_weak
                    .lock()
                    .unwrap()
                    .take()
                    .expect("Controller should be initialized");

                // 将窗口句柄和控制器弱引用保存到 MainWindow 中
                self.pdf_window_handle = Some(handle);
                self.pdf_window_controller = Some(weak_controller);
            }
            Err(e) => {
                error!("MainWindow: 无法开启独立 PDF 窗口: {:?}", e);
            }
        }
    }
}
