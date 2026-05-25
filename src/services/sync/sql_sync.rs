//! 元数据同步服务模块 (`MySQL`)
//!
//! 前端服务层：负责文献元数据与 `MySQL` 数据库的双向同步编排。

use crate::config::AppConfig;
use crate::services::MainApp;
use crate::services::sync::SyncStatus;
use anyhow::Result;
use database::{Database, MySqlManager};
use log::{debug, error, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 元数据同步服务
pub struct SQLSyncService {
    db: Arc<Database>,
    mysql: Arc<MySqlManager>,
    sync_status: Arc<Mutex<SyncStatus>>,
    attachment_dir: Arc<std::sync::RwLock<std::path::PathBuf>>,
}

impl std::fmt::Debug for SQLSyncService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SQLSyncService")
            .field("db", &self.db)
            .finish()
    }
}

impl SQLSyncService {
    pub fn new(db: Arc<Database>, config: &AppConfig) -> Result<Self> {
        info!("存储管理: [SQL] 正在初始化存储管理器...");

        let manager = Self {
            db,
            mysql: Arc::new(MySqlManager::new(config.database.clone())),
            sync_status: Arc::new(Mutex::new(SyncStatus::Idle)),
            attachment_dir: Arc::new(std::sync::RwLock::new(config.attachment_path.clone())),
        };

        info!("存储管理: [SQL] 初始化完成");
        Ok(manager)
    }

    pub fn update_config(&self, config: &AppConfig) {
        info!("存储管理: [SQL] 正在更新同步配置...");
        let old_path = self.attachment_dir.read().map(|r| r.clone()).ok();
        if let Some(pool) = self.mysql.update_config(config.database.clone()) {
            crate::RUNTIME.spawn(async move {
                if let Err(e) = pool.disconnect().await {
                    error!("MySQL: 断开旧连接池失败: {e}");
                }
            });
        }
        if let Ok(mut w) = self.attachment_dir.write() {
            let new_path = config.attachment_path.clone();
            if Some(&new_path) != old_path.as_ref() {
                debug!("存储管理: [SQL] 附件目录已变更: {:?} -> {:?}", old_path, new_path);
            }
            *w = new_path;
        }
    }

    pub fn perform_full_sync(
        &self,
        app: Arc<MainApp>,
        auto_sync_paused: Arc<Mutex<bool>>,
        allowed_attachment_ids: Option<Vec<String>>,
    ) -> tokio::task::JoinHandle<()> {
        let status_mutex = self.sync_status.clone();
        let mysql = self.mysql.clone();
        let app_clone = app.clone();
        let db = self.db.clone();
        let attachment_dir = self.attachment_dir.clone();

        crate::RUNTIME.spawn(async move {
            {
                let status = status_mutex.lock().await;
                if *status == SyncStatus::Syncing {
                    warn!("存储管理: 另一次同步已在运行中，跳过本次全量同步请求");
                    return;
                }
            }

            *status_mutex.lock().await = SyncStatus::Syncing;
            if let Ok(mut state) = app_clone.sync_state.lock() {
                state.sync_status = SyncStatus::Syncing;
            }
            app_clone.notify_ui_changed();

            info!("存储管理: 开始执行全量同步 (MySQL)...");
            let start = std::time::Instant::now();

            let base_path = attachment_dir.read().unwrap().clone();
            let sync_result = mysql
                .sync_metadata(db, &base_path, allowed_attachment_ids.as_deref())
                .await;

            let elapsed = start.elapsed();

            let final_status = match sync_result {
                Ok(conflicts) => {
                    if conflicts.is_empty() {
                        info!("存储管理: 元数据同步成功 (耗时: {elapsed:?})，正在刷新内存数据...");
                        let _ = app_clone.refresh_all_data();
                        SyncStatus::Idle
                    } else {
                        warn!(
                            "存储管理: 元数据同步完成 (耗时: {elapsed:?})，但发现 {} 个冲突项",
                            conflicts.len()
                        );
                        SyncStatus::Conflict(conflicts)
                    }
                }
                Err(e) => {
                    let err_msg = e.to_string();
                    error!("存储管理: 元数据同步失败 (耗时: {elapsed:?}): {err_msg}");
                    *auto_sync_paused.lock().await = true;
                    SyncStatus::Error(err_msg)
                }
            };

            *status_mutex.lock().await = final_status.clone();
            if let Ok(mut state) = app_clone.sync_state.lock() {
                state.sync_status = final_status;
            }
            app_clone.notify_ui_changed();
            info!("存储管理: 全量同步流程执行结束");
        })
    }

    pub async fn test_mysql_config(&self, config: database::DatabaseConfig) -> anyhow::Result<()> {
        let host = config.host.clone();
        debug!("存储管理: 正在测试 MySQL 连接配置 (Host: {host})");
        let handle =
            crate::RUNTIME.spawn(async move { MySqlManager::new(config).test_connection().await });
        let result = handle.await.map_err(|e| anyhow::anyhow!("任务失败: {e}"))?;
        match &result {
            Ok(()) => info!("存储管理: MySQL 连接测试通过 (Host: {host})"),
            Err(e) => error!("存储管理: MySQL 连接测试失败 (Host: {host}): {e}"),
        }
        result
    }

    pub async fn clear_remote_data(&self) -> anyhow::Result<()> {
        info!("存储管理: 开始清空远程数据...");
        let mysql = self.mysql.clone();
        let handle = crate::RUNTIME.spawn(async move {
            debug!("存储管理: [SQL] 正在清空 MySQL 端所有数据...");
            let r = mysql.clear_all_data().await;
            match &r {
                Ok(()) => info!("存储管理: [SQL] 远程数据已清空"),
                Err(e) => error!("存储管理: [SQL] 清空远程数据失败: {e}"),
            }
            r
        });
        handle.await.map_err(|e| anyhow::anyhow!("任务失败: {e}"))?
    }

    pub async fn get_sync_status(&self) -> SyncStatus {
        self.sync_status.lock().await.clone()
    }
}
