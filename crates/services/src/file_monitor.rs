use log::{debug, error, info, warn};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parser::csl::registry::REGISTRY as CSL_REGISTRY;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

#[derive(Debug)]
pub enum FileEvent {
    AttachmentChanged(PathBuf),
    /// 变更的主题文件路径列表（由接收方负责重载，避免 services 反向依赖 ui）
    ThemeChanged(Vec<PathBuf>),
    StylesChanged,
}

pub struct FileMonitorService {
    _watcher: RecommendedWatcher,
}

impl FileMonitorService {
    pub fn new(
        attachments_dir: PathBuf,
        themes_dir: PathBuf,
        csl_dir: PathBuf,
    ) -> Option<(Self, UnboundedReceiver<FileEvent>)> {
        let (notify_tx, notify_rx) = channel();
        let (event_tx, event_rx) = unbounded_channel();

        let mut watcher = match RecommendedWatcher::new(notify_tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                error!("文件监控: 无法创建监听器: {e}");
                return None;
            }
        };

        let _ = std::fs::create_dir_all(&themes_dir);
        let _ = std::fs::create_dir_all(&csl_dir);

        let themes_dir = themes_dir.canonicalize().unwrap_or(themes_dir);
        let csl_dir = csl_dir.canonicalize().unwrap_or(csl_dir);

        let dirs: [(&str, &PathBuf); 3] = [
            ("附件", &attachments_dir),
            ("主题", &themes_dir),
            ("CSL", &csl_dir),
        ];
        let mut any_watched = false;
        for (name, dir) in &dirs {
            if !dir.exists() {
                warn!("文件监控: {name}目录不存在，跳过监听: {}", dir.display());
                continue;
            }
            match watcher.watch(dir, RecursiveMode::Recursive) {
                Ok(()) => {
                    info!("文件监控: 开始监听{name}目录: {}", dir.display());
                    any_watched = true;
                }
                Err(e) => {
                    error!("文件监控: 无法监听{name}目录 {}: {}", dir.display(), e);
                }
            }
        }

        if !any_watched {
            warn!("文件监控: 没有可监听的目录，服务未启动");
            return None;
        }

        thread::spawn(move || {
            let path_debounce = Duration::from_millis(500);
            let category_debounce = Duration::from_millis(1000);
            let mut debounce_map: HashMap<PathBuf, Instant> = HashMap::new();
            let mut last_theme_notify: Option<Instant> = None;
            let mut last_styles_notify: Option<Instant> = None;
            let mut theme_paths: Vec<PathBuf> = Vec::new();

            loop {
                match notify_rx.recv() {
                    Ok(Ok(event)) => {
                        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                            continue;
                        }

                        let mut theme_changed_in_batch = false;
                        let mut styles_changed_in_batch = false;

                        for path in event.paths {
                            if path.is_dir() {
                                continue;
                            }
                            if let Some(name) = path.file_name() {
                                let s = name.to_string_lossy();
                                if s.starts_with('.') || s.ends_with(".tmp") {
                                    continue;
                                }
                            }

                            let now = Instant::now();

                            if path.starts_with(&attachments_dir) {
                                if let Some(last) = debounce_map.get(&path)
                                    && now.duration_since(*last) < path_debounce
                                {
                                    debug!("文件监控: 附件防抖跳过: {}", path.display());
                                    continue;
                                }
                                debounce_map.insert(path.clone(), now);
                                debug!("文件监控: 附件变更: {}", path.display());
                                let _ = event_tx.send(FileEvent::AttachmentChanged(path));
                            } else if path.starts_with(&themes_dir) {
                                info!("文件监控: 主题文件变更: {}", path.display());
                                theme_changed_in_batch = true;
                                theme_paths.push(path.clone());
                            } else if path.starts_with(&csl_dir) {
                                info!("文件监控: CSL 样式文件变更: {}", path.display());
                                if let Ok(mut registry) = CSL_REGISTRY.write() {
                                    registry.reload_style_from_file(&path);
                                    styles_changed_in_batch = true;
                                }
                            }
                        }

                        let now = Instant::now();
                        if theme_changed_in_batch {
                            if let Some(last) = last_theme_notify
                                && now.duration_since(last) < category_debounce
                            {
                                debug!("文件监控: 主题通知防抖跳过");
                            } else {
                                last_theme_notify = Some(now);
                                if !theme_paths.is_empty() {
                                    let _ =
                                        event_tx.send(FileEvent::ThemeChanged(theme_paths.clone()));
                                }
                            }
                        }
                        if styles_changed_in_batch {
                            if let Some(last) = last_styles_notify
                                && now.duration_since(last) < category_debounce
                            {
                                debug!("文件监控: CSL 通知防抖跳过");
                            } else {
                                last_styles_notify = Some(now);
                                let _ = event_tx.send(FileEvent::StylesChanged);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        error!("文件监控: 监听错误: {e:?}");
                    }
                    Err(_) => {
                        info!("文件监控: 通道关闭，服务停止");
                        break;
                    }
                }
            }
        });

        Some((Self { _watcher: watcher }, event_rx))
    }
}
