use std::collections::HashSet;

use crate::RUNTIME;
use crate::config::AppConfig;
use crate::services::sync::SyncStatus;
use crate::services::{
    AttachmentService, FetcherService, FolderService, LiteratureService, TagService,
    TranslationService,
};
use crate::services::{CCFService, FeedService};
use crate::services::{SyncService, filename};
use crate::ui::views::main_window::FetchSource;
use anyhow::{Result, anyhow};
use database::Database;
use database::constructors::*;
use i18n::{I18nKey, Language, t};
use log::{debug, error, info, warn};
use models::{Attachment, FeedType, FolderType, Literature};
use parser::csl::{StyleInfo, available_styles, format_bibliography};
use parser::export::ExportManager;
use parser::normalize::*;
use std::{
    path::Path,
    process::Command,
    sync::{Arc, Mutex},
};
use sync::LocalFileManager;
use uuid::Uuid;

/// 同步状态（跨线程共享，从 Tokio 写入，UI 读取）
#[derive(Debug, Clone)]
pub struct SyncStateInner {
    pub sync_status: SyncStatus,
    pub attachment_sync_status: SyncStatus,
    pub sync_conflict_groups: Option<Vec<Vec<Literature>>>,
}

impl SyncStateInner {
    pub fn new() -> Self {
        Self {
            sync_status: SyncStatus::Idle,
            attachment_sync_status: SyncStatus::Idle,
            sync_conflict_groups: None,
        }
    }
}

impl Default for SyncStateInner {
    fn default() -> Self {
        Self::new()
    }
}

/// 主应用控制器
///
/// 负责协调数据、UI 和业务逻辑
pub struct MainApp {
    /// 同步状态（跨线程共享）
    pub sync_state: Arc<Mutex<SyncStateInner>>,
    /// 应用配置
    pub config: Mutex<AppConfig>,
    /// 文献服务
    pub literature_service: Arc<LiteratureService>,
    /// 文件夹服务
    pub folder_service: Arc<FolderService>,
    /// 订阅服务
    pub feed_service: Arc<FeedService>,
    /// 标签服务
    pub tag_service: Arc<TagService>,
    /// 附件服务
    pub attachment_service: Arc<AttachmentService>,
    /// 数据库
    pub db: Arc<Database>,
    /// 本地文件管理器
    pub file_manager: LocalFileManager,
    /// 存储与同步管理器 (持久化)
    pub sync_service: Arc<SyncService>,
    /// 解析管理器
    pub fetcher_service: Arc<FetcherService>,
    /// 导出管理器
    pub export_manager: Arc<ExportManager>,
    /// CCF 分级管理器
    pub ccf_service: Arc<CCFService>,
    /// 本地 UI 状态 (内存中)
    pub local_state: Arc<std::sync::RwLock<models::local_state::AppUiState>>,
    /// 本地状态管理器 (数据库)
    pub local_state_manager: Arc<database::LocalStateManager>,
    /// 翻译服务
    pub translation_service: Arc<Mutex<TranslationService>>,
    /// 跨线程通知通道发送端（桥接 GPUI !Send 限制）
    pub refresh_tx: Mutex<Option<tokio::sync::broadcast::Sender<super::data_store::RefreshMsg>>>,
}

// =============================================================================
// --- 1. 核心设施：初始化、配置与通知 ---
// =============================================================================
impl MainApp {
    #[must_use]
    pub fn new(
        config: AppConfig,
        local_state_manager: Arc<database::LocalStateManager>,
        initial_state: models::local_state::AppUiState,
    ) -> (Self, tokio::sync::mpsc::Receiver<()>) {
        info!("开始创建 MainApp 实例...");
        let (backend_name, backend_config_json) =
            if config.webdav.enabled && !initial_state.webdav_password.is_empty() {
                let cfg = serde_json::to_string(&sync::WebDavConfig {
                    enabled: true,
                    endpoint: config.webdav.endpoint.clone(),
                    username: config.webdav.username.clone(),
                    password: initial_state.webdav_password.clone(),
                    remote_path: config.webdav.remote_path.clone(),
                })
                .unwrap_or_default();
                ("webdav", cfg)
            } else if config.google_drive.enabled
                && !initial_state.google_drive_refresh_token.is_empty()
            {
                let cfg = serde_json::to_string(&sync::GoogleDriveConfig {
                    enabled: true,
                    client_id: config.google_drive.client_id.clone(),
                    client_secret: config.google_drive.client_secret.clone(),
                    refresh_token: initial_state.google_drive_refresh_token.clone(),
                })
                .unwrap_or_default();
                ("google_drive", cfg)
            } else {
                ("noop", String::new())
            };
        let on_demand = config.webdav.on_demand;
        info!("MainApp 同步配置: backend={backend_name}, on_demand={on_demand}");
        let (sync_service, sync_rx) =
            SyncService::new(&config, backend_name, &backend_config_json, on_demand)
                .expect("Failed to initialize sync_service");
        let sync_service = Arc::new(sync_service);
        let db = sync_service.db.clone();
        let file_manager = sync_service.file_manager.clone();
        (
            Self {
                sync_state: Arc::new(Mutex::new(SyncStateInner::new())),
                refresh_tx: Mutex::new(None),
                literature_service: Arc::new(LiteratureService::new()),
                attachment_service: Arc::new(AttachmentService::new()),
                folder_service: Arc::new(FolderService::new()),
                tag_service: Arc::new(TagService::new()),
                feed_service: Arc::new(FeedService::new()),
                db,
                file_manager,
                sync_service,
                fetcher_service: Arc::new(FetcherService::new()),
                export_manager: Arc::new(ExportManager::new()),
                ccf_service: Arc::new(CCFService::new()),
                local_state: Arc::new(std::sync::RwLock::new(initial_state.clone())),
                local_state_manager,
                translation_service: Arc::new(Mutex::new(TranslationService::new(
                    &config.translation.engine,
                    &initial_state.translation_keys,
                ))),
                config: Mutex::new(config),
            },
            sync_rx,
        )
    }

    pub fn update_config(&self, new_config: AppConfig) -> Result<()> {
        debug!("MainApp: 正在更新配置...");
        if let Ok(blob) = serde_json::to_string(&new_config) {
            let _ = self.local_state_manager.save_config(&blob);
        }
        let local_state = self.local_state.read().unwrap().clone();
        let (backend_name, backend_config_json) =
            if new_config.webdav.enabled && !local_state.webdav_password.is_empty() {
                let cfg = serde_json::to_string(&sync::WebDavConfig {
                    enabled: true,
                    endpoint: new_config.webdav.endpoint.clone(),
                    username: new_config.webdav.username.clone(),
                    password: local_state.webdav_password.clone(),
                    remote_path: new_config.webdav.remote_path.clone(),
                })
                .unwrap_or_default();
                ("webdav", cfg)
            } else if new_config.google_drive.enabled
                && !local_state.google_drive_refresh_token.is_empty()
            {
                let cfg = serde_json::to_string(&sync::GoogleDriveConfig {
                    enabled: true,
                    client_id: new_config.google_drive.client_id.clone(),
                    client_secret: new_config.google_drive.client_secret.clone(),
                    refresh_token: local_state.google_drive_refresh_token.clone(),
                })
                .unwrap_or_default();
                ("google_drive", cfg)
            } else {
                ("noop", String::new())
            };
        let on_demand = new_config.webdav.on_demand;
        self.sync_service
            .update_config(&new_config, backend_name, &backend_config_json, on_demand);
        let keys = self.local_state.read().unwrap().translation_keys.clone();
        self.translation_service
            .lock()
            .unwrap()
            .switch_engine(&new_config.translation.engine, &keys);
        *self.config.lock().unwrap() = new_config;
        self.notify_ui_changed();
        Ok(())
    }

    pub fn current_language(&self) -> Language {
        self.config
            .lock()
            .unwrap()
            .ui
            .language
            .parse()
            .unwrap_or_default()
    }

    pub fn notify_data_changed(&self) {
        self.sync_service.request_sync();
        if let Some(ref tx) = *self.refresh_tx.lock().unwrap() {
            let _ = tx.send(crate::services::data_store::RefreshMsg::DataChanged);
        }
    }

    pub fn notify_ui_changed(&self) {
        if let Some(ref tx) = *self.refresh_tx.lock().unwrap() {
            let _ = tx.send(crate::services::data_store::RefreshMsg::UiChanged);
        }
    }

    pub fn perform_sync(self: Arc<Self>) {
        self.sync_service.clone().perform_full_sync(self);
    }
    pub fn perform_attachments_sync(self: Arc<Self>) {
        self.sync_service.clone().perform_attachments_sync(self);
    }

    pub async fn test_webdav_config(
        &self,
        endpoint: String,
        username: String,
        password: String,
        remote_path: String,
    ) -> Result<(), String> {
        let config_json = serde_json::to_string(&sync::WebDavConfig {
            enabled: true,
            endpoint,
            username,
            password,
            remote_path,
        })
        .map_err(|e| e.to_string())?;
        self.sync_service
            .test_backend_config("webdav", &config_json)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn test_google_drive_config(
        &self,
        client_id: String,
        client_secret: String,
        refresh_token: String,
    ) -> Result<(), String> {
        let config_json = serde_json::to_string(&sync::GoogleDriveConfig {
            enabled: true,
            client_id,
            client_secret,
            refresh_token,
        })
        .map_err(|e| e.to_string())?;
        self.sync_service
            .test_backend_config("google_drive", &config_json)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn test_mysql_config(&self, config: database::DatabaseConfig) -> Result<(), String> {
        self.sync_service
            .test_mysql_config(config)
            .await
            .map_err(|e| e.to_string())
    }

    /// 内部助手：消除“操作 -> 通知 -> 返回”模板代码
    fn op_notify<R>(&self, op: impl FnOnce() -> Result<R>) -> Result<R> {
        let res = op()?;
        self.notify_data_changed();
        Ok(res)
    }
}

// =============================================================================
// --- 2. 文献与文件夹业务 (CRUD) ---
// =============================================================================
impl MainApp {
    pub fn add_literature(&self, lit: Literature) -> Result<()> {
        debug!(
            "MainApp: 添加文献 '{}' (id={})",
            &lit.title.chars().take(40).collect::<String>(),
            lit.id
        );
        self.op_notify(|| self.literature_service.save_literature(self, lit))?;
        Ok(())
    }

    pub fn update_literature(&self, lit: Literature) -> Result<()> {
        debug!(
            "MainApp: 更新文献 '{}' (id={})",
            &lit.title.chars().take(40).collect::<String>(),
            lit.id
        );
        self.op_notify(|| self.literature_service.update_literature_details(self, lit))
    }

    /// 内部删除实现，不触发 notify（供批量方法复用）
    fn delete_literature_inner(&self, id: &str) -> Result<()> {
        let in_trash = self
            .db
            .get_literature(id)?
            .is_some_and(|lit| lit.folder_ids.contains(&"trash".to_string()));
        if in_trash {
            info!("MainApp: 物理删除文献 (id={id})");
            self.literature_service.delete_literature(self, id)?;
        } else {
            info!("MainApp: 移动文献到回收站 (id={id})");
            self.literature_service
                .set_literature_folders(self, id, vec!["trash".to_string()])?;
        }
        Ok(())
    }

    /// 批量删除，只发一次 notify_data_changed
    pub fn batch_delete_literatures(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        info!("MainApp: 批量删除 {} 篇文献", ids.len());
        for id in ids {
            self.delete_literature_inner(id)?;
        }
        self.notify_data_changed();
        Ok(())
    }

    pub fn delete_literature_by_id(&self, id: &str) -> Result<()> {
        info!("MainApp: 删除单篇文献 (id={id})");
        self.op_notify(|| self.delete_literature_inner(id))
    }

    pub fn empty_trash(&self) -> Result<()> {
        let ids: Vec<String> = self
            .db
            .get_literatures_by_folder("trash")?
            .iter()
            .map(|l| l.id.clone())
            .collect();
        if ids.is_empty() {
            debug!("MainApp: 清空回收站，但回收站为空");
            return Ok(());
        }
        info!("MainApp: 清空回收站，共 {} 篇文献", ids.len());
        self.batch_delete_literatures(&ids)
    }

    pub fn add_folder(&self, parent_id: Option<String>, new_id: Option<String>) -> Result<()> {
        info!("MainApp: 添加文件夹 (parent={parent_id:?})");
        let folder = create_folder(
            new_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            "",
            FolderType::Custom,
        );
        let mut folder = folder;
        folder.parent_id = parent_id;
        self.op_notify(|| self.folder_service.save_folder(self, folder))
    }

    pub fn delete_folder(&self, id: &str) -> Result<()> {
        info!("MainApp: 删除文件夹 (id={id})");
        self.op_notify(|| self.folder_service.delete_folder(self, id, true))
    }
    pub fn rename_folder(&self, id: &str, name: String) -> Result<()> {
        debug!("MainApp: 重命名文件夹 (id={id}) -> '{name}'");
        self.op_notify(|| self.folder_service.update_folder_name(self, id, name))
    }
    pub fn add_literature_to_folder(&self, lit_id: &str, f_id: &str) -> Result<()> {
        debug!("MainApp: 添加文献到文件夹 lit={lit_id}, folder={f_id}");
        self.op_notify(|| {
            self.literature_service
                .add_literature_to_folder(self, lit_id, f_id)
        })
    }

    pub fn remove_literature_from_folder(&self, lit_id: &str, f_id: &str) -> Result<()> {
        debug!("MainApp: 从文件夹移除文献 lit={lit_id}, folder={f_id}");
        self.op_notify(|| {
            self.literature_service
                .remove_literature_from_folder(self, lit_id, f_id)
        })
    }
    pub fn restore_literature(&self, lit_id: &str, target: Option<&str>) -> Result<()> {
        debug!("MainApp: 恢复文献 lit={lit_id}, target={target:?}");
        self.op_notify(|| {
            self.literature_service
                .remove_literature_from_folder(self, lit_id, "trash")?;
            if let Some(f) = target {
                self.literature_service
                    .add_literature_to_folder(self, lit_id, f)?;
            }
            Ok(())
        })
    }

    /// 批量重命名所有文献的主文件
    pub fn batch_rename_files(&self) -> Result<()> {
        warn!("MainApp: batch_rename_files 尚未实现");
        Ok(())
    }

    /// 删除指定的文献集合
    pub fn delete_selected_literatures(&self, ids: Vec<String>) -> Result<()> {
        debug!("MainApp: 删除选中文献集合 ({} 篇)", ids.len());
        self.batch_delete_literatures(&ids)
    }
}

// =============================================================================
// --- 4. 订阅与 Feed 流管理 ---
// =============================================================================
impl MainApp {
    pub fn add_feed(self: Arc<Self>, name: String, url: String, interval: u32) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        info!("MainApp: 添加订阅 name='{name}', url='{url}', interval={interval}h, id={id}");
        let mut feed = create_feed(id.clone(), name, FeedType::Rss);
        feed.url = Some(url);
        feed.update_interval = interval;
        self.op_notify(|| self.feed_service.save_feed(self.as_ref(), feed))?;
        let (app, feed_mgr) = (self.clone(), self.feed_service.clone());
        RUNTIME.spawn(async move {
            info!("MainApp: 启动新订阅的首次刷新 (id={id})");
            let _ = feed_mgr.refresh_feed(app, id).await;
        });
        Ok(())
    }

    pub fn update_feed(&self, id: String, name: String, url: String, interval: u32) -> Result<()> {
        info!("MainApp: 更新订阅 (id={id})");
        let mut feed = self.db.get_feed(&id)?.ok_or_else(|| {
            warn!("MainApp: 更新订阅失败，未找到 (id={id})");
            anyhow!("订阅不存在")
        })?;
        feed.name = name;
        feed.url = Some(url);
        feed.update_interval = interval;
        self.op_notify(|| self.feed_service.save_feed(self, feed))
    }

    pub fn delete_feed(&self, id: &str) -> Result<()> {
        info!("MainApp: 删除订阅 (id={id})");
        self.op_notify(|| self.feed_service.delete_feed(self, id))
    }

    /// 删除指定的订阅条目集合
    pub fn delete_selected_feed_items(&self, ids: Vec<String>) -> Result<()> {
        debug!("MainApp: 批量删除订阅项 ({} 条)", ids.len());
        for id in ids {
            self.feed_service.delete_feed_item(self, &id)?;
        }
        self.notify_data_changed();
        Ok(())
    }

    pub fn add_feed_item_to_library(&self, id: &str) -> Result<String> {
        let item = self.db.get_feed_item(id)?.ok_or_else(|| {
            warn!("MainApp: 添加订阅项到文献库失败，未找到 (id={id})");
            anyhow!("订阅项不存在")
        })?;
        if item.is_added_to_library {
            debug!("MainApp: 订阅项已添加过 (id={id}), 尝试查找已有文献");
            if let Some(lit) = self
                .db
                .get_all_literatures()?
                .iter()
                .find(|l| l.title == item.title)
                .cloned()
            {
                debug!("MainApp: 找到已有文献 id={}", lit.id);
                return Ok(lit.id);
            }
            warn!("MainApp: 订阅项已标记添加但未找到对应文献 (id={id})");
            return Err(anyhow!("文献已添加但未找到对应记录"));
        }
        info!(
            "MainApp: 从订阅项创建文献 (id={id}, title='{}')",
            &item.title.chars().take(40).collect::<String>()
        );
        let lit_id = Uuid::new_v4().to_string();
        let mut lit = create_literature(
            lit_id.clone(),
            item.title.clone(),
            item.literature_type.clone(),
        );
        lit.authors = item.authors.clone();
        lit.year = item.year;
        lit.abstract_text = item.abstract_text.clone();
        lit.doi = item.doi.clone();
        lit.url = item.url.clone();
        sanitize_arxiv_identifiers(&mut lit);
        lit.volume = item.volume.clone();
        lit.issue = item.issue.clone();
        lit.pages = item.pages.clone();
        if let Some(ref j) = item.journal {
            lit.publication = Some(create_publication(
                j.clone(),
                models::PublicationType::Journal,
            ));
        }
        self.op_notify(|| {
            self.literature_service.save_literature(self, lit)?;
            self.feed_service
                .update_feed_item_added_status(self, id, true)
        })?;
        Ok(lit_id)
    }
}

// =============================================================================
// --- 5. 文件、附件与导出 ---
// =============================================================================
impl MainApp {
    pub fn import_file_to_literature(
        &self,
        lit_id: &str,
        path: &Path,
        is_main: bool,
    ) -> Result<()> {
        info!(
            "MainApp: 导入文件到文献 lit={lit_id}, path='{}', is_main={is_main}",
            path.display()
        );
        let lit = self.db.get_literature(lit_id)?.ok_or_else(|| {
            warn!("MainApp: 导入失败，找不到文献 (id={lit_id})");
            anyhow!("找不到文献")
        })?;
        let (last, first) = lit.authors.first().map_or_else(
            || ("Unknown".to_string(), String::new()),
            |a| (a.last_name.clone(), a.first_name.clone()),
        );
        let opts = filename::filename_options_from_path(
            &last,
            &first,
            lit.year,
            &lit.title,
            &lit.publication
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_default(),
            path,
            is_main,
        );
        let name = filename::generate_literature_filename(
            &opts,
            Some(&self.config.lock().unwrap().filename_template.clone()),
        );
        let mut new_lit = lit.clone();
        if is_main {
            for a in &new_lit.attachments {
                if a.is_main {
                    let _ = self.file_manager.trash_file(&a.file_path);
                }
            }
            new_lit.attachments.retain(|a| !a.is_main);
        }
        let result = self.file_manager.upload_file_with_name(path, &name)?;
        let mut att = database::constructors::create_attachment(
            Uuid::new_v4().to_string(),
            lit_id.to_string(),
            result.final_path.to_string_lossy().to_string(),
            result.final_name,
            result.size,
        );
        att.is_main = is_main;
        new_lit.attachments.push(att);
        new_lit.version += 1;
        new_lit.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.op_notify(|| self.literature_service.save_literature(self, new_lit))
    }

    pub fn open_attachment(&self, id: &str) -> Result<()> {
        if let Some(att) = self.get_attachment_by_id(id) {
            let path = Path::new(&att.file_path);
            let config = self.config.lock().unwrap().clone();

            // 捕获通知发送端
            let refresh_tx = self.refresh_tx.lock().unwrap().clone();

            if path.exists() {
                info!("MainApp: 打开附件 (id={id}, path='{}')", att.file_path);
                Self::open_file_with_config(&att.file_path, &config)?;
            } else {
                let att_id = att.id.clone();
                info!(
                    "MainApp: 附件本地不存在，触发远程下载 (id={att_id}, name='{}')",
                    att.file_name
                );
                let sync = self.sync_service.clone();
                let db = self.db.clone();
                // 异步执行下载/修复任务
                RUNTIME.spawn(async move {
                    match sync.download_single_file(&att).await {
                        Ok(changed) => {
                            if changed {
                                info!("MainApp: 附件下载成功并已更新本地记录");
                                if let Some(tx) = &refresh_tx {
                                    let _ = tx
                                        .send(crate::services::data_store::RefreshMsg::DataChanged);
                                }
                                // 同时请求一次后台同步以保持状态一致
                                sync.request_sync();
                            } else {
                                debug!("MainApp: 附件下载返回无变更");
                            }
                            // 最稳妥的方式：重新获取 attachment
                            if let Ok(Some(new_att)) = db.get_attachment(&att.id) {
                                let _ = Self::open_file_with_config(&new_att.file_path, &config);
                            }
                        }
                        Err(e) => {
                            error!("下载/打开附件失败 (id={att_id}): {e}");
                        }
                    }
                });
            }
        } else {
            warn!("MainApp: 打开附件失败，未找到 (id={id})");
        }
        Ok(())
    }

    pub fn open_literature_main_file(&self, id: &str) -> Result<()> {
        debug!("MainApp: 打开文献主文件 (lit_id={id})");
        let att_id = self.db.get_literature(id)?.and_then(|l| {
            l.attachments
                .iter()
                .find(|a| a.is_main)
                .map(|a| a.id.clone())
        });
        if let Some(aid) = att_id {
            self.open_attachment(&aid)?;
        } else {
            debug!("MainApp: 文献无主文件 (lit_id={id})");
        }
        Ok(())
    }

    pub fn reveal_in_explorer(&self, id: &str) -> Result<()> {
        let att = self
            .get_attachment_by_id(id)
            .ok_or_else(|| anyhow!("未找到附件: {id}"))?;
        let path = Path::new(&att.file_path);

        if !path.exists() {
            anyhow::bail!("文件不存在: {}", att.file_path);
        }

        info!("在资源管理器中显示文件: {}", att.file_path);

        #[cfg(target_os = "macos")]
        Command::new("open").arg("-R").arg(&att.file_path).spawn()?;

        #[cfg(target_os = "windows")]
        Command::new("explorer")
            .arg("/select,")
            .arg(&att.file_path)
            .spawn()?;

        #[cfg(target_os = "linux")]
        if let Some(parent) = path.parent() {
            Command::new("xdg-open").arg(parent).spawn()?;
        }

        Ok(())
    }

    pub fn delete_attachment_file(&self, id: &str) -> Result<()> {
        let att = self.db.get_attachment(id)?.ok_or_else(|| {
            warn!("MainApp: 删除附件失败，未找到 (id={id})");
            anyhow!("找不到附件")
        })?;
        info!("MainApp: 删除附件文件 (id={id}, name='{}')", att.file_name);
        let path = att.file_path;
        self.op_notify(|| {
            let _ = self.file_manager.trash_file(&path);
            self.db.delete_attachment(id)?;
            Ok(())
        })
    }

    pub fn get_attachment_by_id(&self, id: &str) -> Option<Attachment> {
        self.db.get_attachment(id).unwrap_or(None)
    }

    /// 判断文件是否应使用外部程序打开（非PDF或启用了外置阅读器时为true）
    pub fn should_use_external_viewer(&self, path: &str) -> bool {
        let is_pdf = path.to_lowercase().ends_with(".pdf");
        if !is_pdf {
            return true;
        }
        let config = self.config.lock().unwrap();
        config.pdf_viewer.use_custom
    }

    fn open_file_with_config(path: &str, config: &AppConfig) -> Result<()> {
        debug!("MainApp: 使用系统打开文件 (path='{path}')");
        let is_pdf = path.to_lowercase().ends_with(".pdf");
        if is_pdf && config.pdf_viewer.use_custom {
            #[cfg(target_os = "macos")]
            if !config.pdf_viewer.macos_app.is_empty() {
                return Ok(Command::new("open")
                    .arg("-a")
                    .arg(&config.pdf_viewer.macos_app)
                    .arg(path)
                    .spawn()
                    .map(|_| ())?);
            }
            #[cfg(target_os = "windows")]
            if !config.pdf_viewer.windows_app.is_empty() {
                return Ok(Command::new(&config.pdf_viewer.windows_app)
                    .arg(path)
                    .spawn()
                    .map(|_| ())?);
            }
        }
        #[cfg(target_os = "macos")]
        Command::new("open").arg(path).spawn()?;
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new("cmd")
                .arg("/c")
                .arg("start")
                .arg("")
                .arg(path)
                .creation_flags(0x08000000)
                .spawn()?;
        }
        #[cfg(target_os = "linux")]
        Command::new("xdg-open").arg(path).spawn()?;
        Ok(())
    }

    pub async fn fetch_metadata_from_source(&self, source: FetchSource) -> Result<Literature> {
        debug!("MainApp: 从外部源获取元数据");
        match source {
            FetchSource::Doi(doi) => self.fetcher_service.parse_doi(&doi).await,
            FetchSource::ArXiv(id) => self.fetcher_service.parse_arxiv(&id).await,
            FetchSource::Dblp(query) => self.fetcher_service.resolve_dblp_best_match(&query).await,
            FetchSource::OpenAlexDoi(doi) => self.fetcher_service.parse_openalex(&doi).await,
            FetchSource::OpenAlexTitle(title) => {
                self.fetcher_service
                    .resolve_openalex_best_match(&title)
                    .await
            }
        }
    }

    pub fn format_selected_literatures(
        &self,
        selected_ids: &HashSet<String>,
        style: &str,
    ) -> Result<String> {
        debug!(
            "MainApp: 格式化参考文献 style='{style}', 选中 {} 篇",
            selected_ids.len()
        );
        let selected: Vec<Literature> = selected_ids
            .iter()
            .filter_map(|id| self.db.get_literature(id).ok().flatten())
            .collect();
        if selected.is_empty() {
            let lang = self
                .config
                .lock()
                .unwrap()
                .ui
                .language
                .parse::<Language>()
                .unwrap_or_default();
            debug!("MainApp: 格式引用无选中文献");
            return Ok(t(I18nKey::NoLiteratureSelected, lang).to_string());
        }
        format_bibliography(&selected, style)
    }

    pub fn available_citation_styles(&self) -> Vec<StyleInfo> {
        let styles = available_styles();
        debug!("MainApp: 获取可用引文样式, 共 {} 种", styles.len());
        styles
    }
    pub fn find_duplicates(&self) -> Vec<Vec<Literature>> {
        let result = self.literature_service.find_duplicates(self);
        let total_dup: usize = result.iter().map(|g| g.len()).sum();
        debug!(
            "MainApp: 查重完成, 发现 {} 组共 {} 篇重复文献",
            result.len(),
            total_dup
        );
        result
    }

    pub fn merge_literature_relations(&self, source_id: &str, target_id: &str) -> Result<()> {
        info!("MainApp: 合并文献关系 source={source_id} -> target={target_id}");
        self.op_notify(|| {
            self.db.merge_literature_relations(source_id, target_id)?;
            Ok(())
        })
    }

    pub fn cleanup_orphaned_files(&self) -> Result<()> {
        info!("MainApp: 清理孤立文件...");
        self.attachment_service.cleanup_orphaned_files(self)
    }
}

// =============================================================================
// --- 6. 智能 UI 动作 (Smart Actions) ---
// =============================================================================
impl MainApp {
    /// 根据 target_id 是否属于选中的 ID 集合，决定批量操作的目标集合。
    pub fn resolve_smart_targets(target_id: &str, selected_ids: &HashSet<String>) -> Vec<String> {
        if selected_ids.contains(target_id) {
            selected_ids.iter().cloned().collect()
        } else {
            vec![target_id.to_string()]
        }
    }

    pub fn smart_delete_literature(&self, id: &str, selected_ids: &HashSet<String>) -> Result<()> {
        let targets = Self::resolve_smart_targets(id, selected_ids);
        self.batch_delete_literatures(&targets)
    }
    pub fn smart_add_literatures_to_folder(
        &self,
        id: &str,
        f: &str,
        selected_ids: &HashSet<String>,
    ) -> Result<()> {
        self.op_notify(|| {
            for aid in Self::resolve_smart_targets(id, selected_ids) {
                self.literature_service
                    .add_literature_to_folder(self, &aid, f)?;
            }
            Ok(())
        })
    }
    pub fn smart_remove_literatures_from_folder(
        &self,
        id: &str,
        f: &str,
        selected_ids: &HashSet<String>,
    ) -> Result<()> {
        self.op_notify(|| {
            for aid in Self::resolve_smart_targets(id, selected_ids) {
                self.literature_service
                    .remove_literature_from_folder(self, &aid, f)?;
            }
            Ok(())
        })
    }
    pub fn smart_restore_literatures(
        &self,
        id: &str,
        f: Option<&str>,
        selected_ids: &HashSet<String>,
    ) -> Result<()> {
        for aid in Self::resolve_smart_targets(id, selected_ids) {
            self.restore_literature(&aid, f)?;
        }
        Ok(())
    }
    pub fn smart_toggle_feed_items_read(
        &self,
        id: &str,
        read: bool,
        selected_ids: &HashSet<String>,
    ) -> Result<()> {
        self.op_notify(|| {
            for aid in Self::resolve_smart_targets(id, selected_ids) {
                self.feed_service
                    .update_feed_item_read_status(self, &aid, read)?;
            }
            Ok(())
        })
    }
    pub fn smart_delete_feed_items(&self, id: &str, selected_ids: &HashSet<String>) -> Result<()> {
        self.op_notify(|| {
            for aid in Self::resolve_smart_targets(id, selected_ids) {
                self.feed_service.delete_feed_item(self, &aid)?;
            }
            Ok(())
        })
    }
}

// =============================================================================
// --- 7. 系统管理与重置 ---
// =============================================================================
impl MainApp {
    pub fn refresh_all_data(&self) -> Result<()> {
        self.notify_data_changed();
        Ok(())
    }

    pub fn clear_local_database(&self) -> Result<()> {
        info!("MainApp: 开始清空本地数据库...");
        self.db.rebuild_schema()?;
        let lang = self
            .config
            .lock()
            .unwrap()
            .ui
            .language
            .parse::<Language>()
            .unwrap_or_default();
        info!("MainApp: 重建默认文件夹和订阅源...");
        for f in [
            database::constructors::create_folder(
                "all",
                t(I18nKey::AllLiterature, lang),
                FolderType::All,
            ),
            database::constructors::create_folder(
                "uncategorized",
                t(I18nKey::Uncategorized, lang),
                FolderType::Uncategorized,
            ),
            database::constructors::create_folder(
                "trash",
                t(I18nKey::Trash, lang),
                FolderType::Trash,
            ),
        ] {
            let _ = self.db.insert_folder(&f);
        }
        for f in [
            database::constructors::create_feed(
                "all_subs",
                t(I18nKey::AllSubscription, lang),
                FeedType::Rss,
            ),
            database::constructors::create_feed("unread", t(I18nKey::Unread, lang), FeedType::Rss),
        ] {
            let _ = self.db.insert_feed(&f);
        }
        info!("MainApp: 本地数据库清空完成");
        Ok(())
    }
}
