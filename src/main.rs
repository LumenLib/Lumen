// Windows GUI 应用配置：不显示控制台窗口
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use database::LocalStateManager;
use env_logger::{Builder, Target};
use gpui::{
    App, AppContext, Application, AsyncApp, Bounds, KeyBinding, Menu, MenuItem, Point,
    TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use gpui_component::Root;
use i18n::{I18nKey, Language, t};
use log::{LevelFilter, debug, error, info, logger};
use lumen::actions::{CloseWindow, Quit, ToggleFullscreen};
use lumen::{
    RUNTIME,
    assets::Assets,
    config::{AppConfig, get_app_root_dir},
    config_store::ConfigStore,
    services::MainApp,
    services::data::{SortField, SortOrder},
    services::data_store::DataStore,
    services::file_monitor::{FileEvent, FileMonitorService},
    ui::{
        theme_manager::LOADER,
        views::main_window::{MainWindow, ShowAbout},
    },
};
use parser::csl::registry::REGISTRY;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::IntoRawHandle;
use std::sync::LazyLock;
use std::{
    fs::{OpenOptions, create_dir_all},
    io::Write,
    panic::set_hook,
    path::Path,
    sync::{Arc, atomic::Ordering},
};

/// 重定向 stderr 到应用日志文件
/// 用于捕获 panic 信息和系统层面的错误输出
#[cfg(unix)]
fn setup_stderr_redirection(log_path: &std::path::Path) {
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let fd = file.as_raw_fd();
        unsafe {
            libc::dup2(fd, libc::STDERR_FILENO);
        }
        info!("系统 stderr 已合并重定向至应用日志文件");
    } else {
        eprintln!("无法重定向系统 stderr");
    }
}

#[cfg(windows)]
fn setup_stderr_redirection(log_path: &std::path::Path) {
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(log_path) {
        unsafe {
            let handle = file.into_raw_handle();
            // 0 is typically text mode, usually safe for logs. O_APPEND equivalent might be needed if not implicit?
            // Windows CRT append mode behavior with fd open might vary, but we opened the file with append option.
            let fd = libc::open_osfhandle(handle as isize, 0);
            if fd != -1 {
                libc::dup2(fd, 2); // 2 is stderr
            }
        }
        info!("系统 stderr 已合并重定向至应用日志文件");
    } else {
        eprintln!("无法重定向系统 stderr");
    }
}

#[cfg(not(any(unix, windows)))]
fn setup_stderr_redirection(_: &std::path::Path) {}

fn setup_panic_hook() {
    set_hook(Box::new(|panic_info| {
        let location = panic_info.location().map_or_else(
            || "unknown".to_string(),
            |l| format!("{}:{}", l.file(), l.line()),
        );
        let payload = panic_info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            *s
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.as_str()
        } else {
            "Box<Any>"
        };

        let backtrace = std::backtrace::Backtrace::capture();
        error!("CRITICAL: 应用发生崩溃 (Panic)!");
        error!("位置: {location}");
        error!("信息: {message}");
        error!("堆栈跟踪:\n{backtrace}");

        // 确保日志被刷入磁盘
        logger().flush();
    }));
}

fn init_logger_with_path(config: &AppConfig, log_path: &Path) {
    // 确保日志目录存在
    if let Some(parent) = log_path.parent() {
        let _ = create_dir_all(parent);
    }

    // 使用追加模式打开日志文件，防止启动时抹除崩溃日志
    let target_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .expect("无法打开日志文件");

    let target = Box::new(target_file);
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter_level(LevelFilter::Trace) // 设到最宽松，运行时用 set_max_level_filter 控制
        .target(Target::Pipe(target))
        .init();
    // 将实际日志级别控制在配置值
    log::set_max_level(
        config
            .log_level
            .parse::<LevelFilter>()
            .unwrap_or(LevelFilter::Info),
    );

    info!("----------------------------------------------------------------");
    info!("日志系统已启动 (追加模式)，日志文件: {log_path:?}");
}

fn main() {
    // 0. 设置崩溃捕获
    setup_panic_hook();

    // 1. 初始化本地状态管理器 (state.db) — 提前到配置加载之前
    let config_dir = get_app_root_dir();
    let local_state_manager = Arc::new(LocalStateManager::new(config_dir));

    if let Err(e) = local_state_manager.init() {
        eprintln!("[FATAL] 本地状态数据库初始化失败: {e:?}");
        error!("无法初始化本地状态数据库: {e}");
    }

    // 2. 从 state.db 加载配置，不存在则创建默认配置并写入
    let config: AppConfig = local_state_manager
        .load_config()
        .ok()
        .flatten()
        .and_then(|blob| serde_json::from_str(&blob).ok())
        .unwrap_or_else(|| {
            let default = AppConfig::default();
            if let Ok(blob) = serde_json::to_string(&default) {
                let _ = local_state_manager.save_config(&blob);
            }
            default
        });

    // 应用代理环境变量
    lumen::config::apply_proxy_config(&config.proxy);

    // 确保数据目录存在
    if let Err(e) = config.ensure_dirs() {
        eprintln!("无法创建应用目录: {e}");
    }

    // 4. 加载初始 UI 状态
    let initial_state = local_state_manager.load_all().unwrap_or_else(|e| {
        error!("加载本地状态失败: {e}, 将使用默认状态");
        Default::default()
    });

    // 5. 加载 CSL 样式
    {
        match REGISTRY.write() {
            Ok(mut registry) => {
                registry.load_from_dir(&config.csl_dir());
            }
            Err(e) => {
                eprintln!("无法获取 CSL 注册表锁: {e}");
            }
        }
    }

    // 6. 加载主题 (从用户目录)
    {
        if let Ok(mut loader) = LOADER.write() {
            let _ = loader.load_all(&config.themes_dir());
        }
    }

    // 7. 初始化日志 (依赖配置)
    config.clean_old_logs();

    let log_path = config.get_current_log_path();
    init_logger_with_path(&config, &log_path);

    // 8. 重定向 stderr
    setup_stderr_redirection(&log_path);

    // Ensure runtime is initialized
    LazyLock::force(&RUNTIME);
    info!("开始初始化应用...");

    Application::new().with_assets(Assets).run({
        let local_state_manager = local_state_manager.clone();
        move |cx: &mut App| {
            // 1. 初始化 UI 组件库环境
            gpui_component::init(cx);

            // 1.1 注册 ConfigStore Global（配置访问的统一入口）
            ConfigStore::load_and_set(&local_state_manager, cx);

            // 1.1.1 注册配置变更观察者 (观察者模式迁移)
            cx.observe_global::<ConfigStore>(|cx| {
                let config = cx.global::<ConfigStore>().inner.clone();
                let scale_val = config.ui.ui_scale;

                // 应用代理环境变量
                lumen::config::apply_proxy_config(&config.proxy);

                // 应用主题到全局 Theme
                lumen::ui::apply_theme(
                    &config.ui.theme_mode,
                    &config.ui.theme_style,
                    scale_val,
                    cx,
                );

                // 同步更新所有窗口的 rem_size（各窗口自行通过 observe_global 触发 cx.notify）
                for window_handle in cx.windows() {
                    let _ = cx.update_window(window_handle, move |_, window, _| {
                        window.set_rem_size(px(16.0 * scale_val));
                    });
                }
            })
            .detach();

            // 1.2 初始化 PDF 阅读器模块 (可选)
            info!("PDF Viewer 模块已加载");

            // 1.5 设置菜单 (MacOS全屏呼出菜单栏依赖菜单配置)
            let lang = config.ui.language.parse::<Language>().unwrap_or_default();

            cx.set_menus(vec![
                Menu {
                    name: "Lumen".into(),
                    items: vec![
                        MenuItem::action(t(I18nKey::About, lang), ShowAbout),
                        MenuItem::separator(),
                        MenuItem::action(t(I18nKey::Quit, lang), Quit),
                    ],
                },
                Menu {
                    name: t(I18nKey::Library, lang).into(), // 使用"文献库"作为 File 菜单的国际化替代或保持逻辑
                    items: vec![
                        MenuItem::action("Close Window", CloseWindow), // 添加关闭窗口菜单项
                        MenuItem::action(t(I18nKey::Quit, lang), Quit),
                    ],
                },
            ]);

            // 2. 初始化应用全局控制器
            let (app_controller_struct, sync_rx) = MainApp::new(
                config.clone(),
                local_state_manager.clone(),
                initial_state.clone(),
            );

            // 2.1.1 初始化 DataStore GPUI Entity（领域数据的新权威源）
            let data_store: lumen::services::data_store::DataStoreEntity =
                cx.new(|_cx| DataStore::new(app_controller_struct.db.clone()));
            data_store.update(cx, |store, cx| {
                if let Err(e) = store.refresh_from_db(cx) {
                    error!("DataStore: refresh_from_db 失败: {e}");
                }
            });

            // 2.2 初始化 NotificationBus Global（通知系统总线）
            cx.set_global(lumen::notification_bus::NotificationBus::new());

            // 2.3 初始化 UiState Global 并应用持久化的初始状态
            cx.set_global(lumen::services::ui_state::UiState::new());
            {
                let state = cx.global_mut::<lumen::services::ui_state::UiState>();
                if let Some(sidebar_item) = &initial_state.selected_sidebar_item {
                    if let Some(folder_id) = sidebar_item.strip_prefix("folder:") {
                        state.selected_folder_id = Some(folder_id.to_string());
                    } else if let Some(tag_id) = sidebar_item.strip_prefix("tag:") {
                        state.selected_tag_id = Some(tag_id.to_string());
                    }
                }
                if let Some(field) = &initial_state.sort_field {
                    state.sort_field = match field.as_str() {
                        "Title" => SortField::Title,
                        "Author" => SortField::Author,
                        "Year" => SortField::Year,
                        "Journal" => SortField::Journal,
                        _ => SortField::Title,
                    };
                }
                state.sort_order = if initial_state.sort_asc {
                    SortOrder::Ascending
                } else {
                    SortOrder::Descending
                };
            }

            let app_controller = Arc::new(app_controller_struct);

            // 启动自动同步后台循环
            app_controller
                .sync_service
                .clone()
                .start_auto_sync_loop(app_controller.clone(), sync_rx);

            // 启动订阅后台更新循环
            app_controller
                .feed_service
                .clone()
                .start_background_loop(app_controller.clone());

            // 3. 绑定退出快捷键
            let mut key_bindings = vec![
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-q"
                    } else {
                        "ctrl-q"
                    },
                    Quit,
                    None,
                ),
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-w"
                    } else {
                        "ctrl-w"
                    },
                    CloseWindow,
                    None,
                ),
            ];

            // 仅在 macOS 上启用全屏快捷键
            if cfg!(target_os = "macos") {
                key_bindings.push(KeyBinding::new("ctrl-cmd-f", ToggleFullscreen, None));
                info!("注册 macOS 全屏快捷键: ctrl-cmd-f");
            } else {
                info!("Windows/Linux 暂不启用全屏快捷键 (框架兼容性限制)");
            }

            cx.bind_keys(key_bindings);

            // 4. 注册退出处理器 (在此处保存状态)
            let lsm_for_quit = local_state_manager.clone();
            let app_for_quit = app_controller.clone();

            cx.on_action(move |_: &Quit, cx| {
                if let Ok(state) = app_for_quit.local_state.read()
                    && let Err(e) = lsm_for_quit.save_all(&state)
                {
                    error!("保存本地状态失败: {e}");
                }
                cx.quit();
            });

            // 注册 CloseWindow 处理器
            cx.on_action(move |_: &CloseWindow, cx| {
                info!("触发 CloseWindow Action (Cmd+W)");
                if let Some(window) = cx.active_window() {
                    cx.update_window(window, |_, window, _| {
                        window.remove_window();
                    })
                    .ok();
                }
            });

            // 注册 ToggleFullscreen 处理器
            cx.on_action(move |_: &ToggleFullscreen, cx| {
                info!("触发 ToggleFullscreen Action");
                if let Some(window) = cx.active_window() {
                    cx.update_window(window, |_, window, _| {
                        window.toggle_fullscreen();
                    })
                    .ok();
                }
            });

            // 5. 打开主窗口并使用 Root 包裹
            // 获取主显示器信息，计算自适应窗口大小
            let display = cx.primary_display();
            let screen_size = display
                .as_ref()
                .map_or_else(|| size(px(1920.0), px(1080.0)), |d| d.bounds().size);

            // 最小窗口尺寸
            let min_width = 900.0_f32;
            let min_height = 600.0_f32;

            // 根据屏幕尺寸计算默认窗口大小
            // 策略：窗口占屏幕可用区域的 75%，但有最小和最大限制
            let screen_width = f32::from(screen_size.width);
            let screen_height = f32::from(screen_size.height);

            let default_width = (screen_width * 0.75).clamp(min_width, 1600.0);
            let default_height = (screen_height * 0.75).clamp(min_height, 1000.0);

            // 从保存的状态恢复窗口尺寸，或使用计算的默认值
            let window_state = &initial_state.window_state;

            let width = window_state
                .width
                .map_or(default_width, |w| (w as f32).clamp(min_width, screen_width));
            let height = window_state.height.map_or(default_height, |h| {
                (h as f32).clamp(min_height, screen_height)
            });

            // 验证保存的位置是否仍在屏幕范围内
            let bounds = if let (Some(x), Some(y)) = (window_state.x, window_state.y) {
                let x = x as f32;
                let y = y as f32;
                // 确保窗口至少有 100px 在屏幕内可见
                let visible_margin = 100.0;
                if x > -width + visible_margin
                    && x < screen_width - visible_margin
                    && y > -height + visible_margin
                    && y < screen_height - visible_margin
                {
                    // 恢复保存的位置和尺寸
                    Bounds {
                        origin: Point::new(px(x), px(y)),
                        size: size(px(width), px(height)),
                    }
                } else {
                    // 位置无效，居中显示
                    Bounds::centered(None, size(px(width), px(height)), cx)
                }
            } else {
                // 居中显示
                Bounds::centered(None, size(px(width), px(height)), cx)
            };

            let should_max = window_state.is_maximized || window_state.width.is_none();

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(if window_state.is_fullscreen {
                        WindowBounds::Fullscreen(bounds)
                    } else {
                        WindowBounds::Windowed(bounds)
                    }),
                    window_min_size: Some(size(px(min_width), px(min_height))),
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: Some(Point::new(px(14.0), px(16.0))),
                    }),
                    ..Default::default()
                },
                {
                    let app_controller = app_controller.clone();
                    let lsm_for_close = local_state_manager.clone();
                    let _ui_scale = config.ui.ui_scale;

                    let data_store_for_window = data_store.clone();
                    move |window, cx| {
                        // 全局唯一退出标记，防止双重 cx.quit()
                        let quit_triggered = Arc::new(std::sync::atomic::AtomicBool::new(false));

                        let main_window = cx.new(|cx| {
                            MainWindow::new(
                                app_controller.clone(),
                                data_store_for_window.clone(),
                                window,
                                cx,
                            )
                        });

                        // 监听窗口尺寸变化，实时更新本地状态（内存）
                        let app_ctrl_bounds = app_controller.clone();
                        let bounds_subscription = main_window.update(cx, |_, cx| {
                            cx.observe_window_bounds(window, move |_, window, _| {
                                let bounds = window.bounds();
                                let is_maximized = window.is_maximized();
                                let is_fullscreen = window.is_fullscreen();
                                if let Ok(mut state) = app_ctrl_bounds.local_state.write() {
                                    if !is_maximized && !is_fullscreen {
                                        state.window_state.width =
                                            Some(f64::from(bounds.size.width));
                                        state.window_state.height =
                                            Some(f64::from(bounds.size.height));
                                        state.window_state.x = Some(f64::from(bounds.origin.x));
                                        state.window_state.y = Some(f64::from(bounds.origin.y));
                                    }
                                    state.window_state.is_maximized = is_maximized;
                                    state.window_state.is_fullscreen = is_fullscreen;
                                }
                            })
                        });

                        // 延迟到窗口完全初始化后再最大化（借鉴 Zed 的 defer 策略）
                        if should_max {
                            window.defer(cx, |window, _cx| {
                                window.zoom_window();
                            });
                        }

                        // 增加主窗口实体释放监听，作为窗口关闭退出的可靠补充
                        // 这解决了 macOS 下窗口关闭时 cx.windows() 可能仍包含已关闭窗口的问题
                        let lsm_for_observe = local_state_manager.clone();
                        let app_ctrl_for_observe = app_controller.clone();
                        let quit_flag_for_observe = quit_triggered.clone();
                        cx.observe_release(&main_window, move |_, cx| {
                            info!("主窗口实体已释放 (MainWindow observe_release)");
                            // 使用 compare_exchange 确保 quit() 只调用一次
                            if quit_flag_for_observe
                                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                                .is_ok()
                            {
                                if let Ok(state) = app_ctrl_for_observe.local_state.read()
                                    && let Err(e) = lsm_for_observe.save_all(&state)
                                {
                                    error!("保存本地状态失败: {e}");
                                }
                                cx.quit();
                            }
                        })
                        .detach();

                        // 监听窗口关闭事件，保存状态到数据库并退出应用
                        let lsm = lsm_for_close.clone();
                        let app_ctrl = app_controller.clone();
                        let main_window_handle = window.window_handle();
                        let quit_flag_for_close = quit_triggered.clone();

                        let close_subscription = cx.on_window_closed(move |cx| {
                            debug!("WINDOW_CLOSE: 窗口关闭事件触发");

                            // 使用 cx.windows() 而非 cx.window_stack()，因为 stack 在某些平台（如 Windows）
                            // 可能在窗口关闭时的行为不符合预期（例如返回空）。
                            // cx.windows() 返回当前应用所有活跃窗口的句柄列表。
                            let windows = cx.windows();
                            let window_count = windows.len();

                            // 检查主窗口句柄是否在当前的窗口列表中
                            let main_any_handle: gpui::AnyWindowHandle = main_window_handle;
                            let has_main = windows.contains(&main_any_handle);

                            debug!("WINDOW_CLOSE: 当前窗口数={window_count}, 主窗口是否在列表={has_main}");

                            // 统一跨平台逻辑：
                            // 1. 如果主窗口不在了 (!has_main)，说明主窗口已关闭 -> 退出。
                            // 2. 如果窗口总数为 0，说明所有窗口都关了 -> 退出。
                            let should_quit = window_count == 0 || !has_main;

                            if should_quit {
                                // 使用 compare_exchange 确保 quit() 只调用一次
                                if quit_flag_for_close
                                    .compare_exchange(
                                        false,
                                        true,
                                        Ordering::SeqCst,
                                        Ordering::SeqCst,
                                    )
                                    .is_ok()
                                {
                                    if let Ok(state) = app_ctrl.local_state.read()
                                        && let Err(e) = lsm.save_all(&state)
                                    {
                                        error!("保存本地状态失败: {e}");
                                    }
                                    info!(
                                        "满足退出条件 (count={} or main_lost={})，执行 cx.quit()",
                                        window_count, !has_main
                                    );
                                    cx.quit();
                                }
                            }
                        });

                        // 将订阅存储到 MainWindow 中以保持其生命周期
                        main_window.update(cx, |mw, _| {
                            mw.bounds_subscription = Some(bounds_subscription);
                            mw.close_subscription = Some(close_subscription);
                        });

                        cx.new(|cx| Root::new(main_window.clone(), window, cx))
                    }
                },
            )
            .expect("无法打开窗口");

            // 窗口创建后初始化统一文件监控服务
            let attachments_dir = app_controller
                .sync_service
                .file_manager
                .get_attachments_dir();
            let app_for_monitor = app_controller.clone();
            if let Some((watcher, mut rx)) = FileMonitorService::new(
                attachments_dir,
                config.themes_dir(),
                config.csl_dir(),
            ) {
                cx.spawn(|wcx: &mut AsyncApp| {
                    let wcx = wcx.clone();
                    async move {
                        let _keep = watcher;
                        while let Some(event) = rx.recv().await {
                            match event {
                                FileEvent::AttachmentChanged(path) => {
                                    let path_str = path.to_string_lossy().to_string();
                                    let db = &app_for_monitor.db;

                                    let mut found_att = db
                                        .get_attachment_by_file_path(&path_str)
                                        .ok()
                                        .flatten();

                                    if found_att.is_none()
                                        && let Some(file_name) =
                                            path.file_name().and_then(|n| n.to_str())
                                        && let Ok(candidates) =
                                            db.get_attachments_by_file_name(file_name)
                                    {
                                        let changed_canonical =
                                            std::fs::canonicalize(&path).ok();
                                        for att in candidates {
                                            let db_canonical = std::fs::canonicalize(
                                                std::path::Path::new(&att.file_path),
                                            )
                                            .ok();
                                            if let (Some(p1), Some(p2)) =
                                                (&changed_canonical, &db_canonical)
                                                && p1 == p2
                                            {
                                                found_att = Some(att);
                                                break;
                                            }
                                        }
                                    }

                                    if let Some(att) = found_att {
                                        info!(
                                            "文件监控: 附件 [{}] 检测到变更，标记为需要同步",
                                            att.id
                                        );
                                        if let Err(e) = db.mark_attachment_dirty(&att.id) {
                                            error!("文件监控: 标记附件失败: {e}");
                                        } else {
                                            app_for_monitor.notify_data_changed();
                                        }
                                    } else {
                                        debug!("文件监控: 未找到对应附件记录: {path_str}");
                                    }
                                }
                                FileEvent::ThemeChanged | FileEvent::StylesChanged => {
                                    let (mode, style, scale) = {
                                        let config = app_for_monitor.config.lock().unwrap();
                                        (
                                            config.ui.theme_mode.clone(),
                                            config.ui.theme_style.clone(),
                                            config.ui.ui_scale,
                                        )
                                    };
                                    let _ = wcx.update(|cx: &mut App| {
                                        lumen::ui::apply_theme(&mode, &style, scale, cx);
                                    });
                                }
                            }
                        }
                    }
                })
                .detach();
            }

            cx.activate(true);
            info!("应用启动完成");
        }
    })
}
