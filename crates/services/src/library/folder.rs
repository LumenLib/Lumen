use anyhow::Result;
use database::Database;
use log::{debug, error, info};
/// 数据库操作单例管理器
///
/// 负责协调持久化存储与内存数据的同步
use models::Folder;

pub struct FolderService;

impl FolderService {
    #[must_use]
    pub fn new() -> Self {
        debug!("文件夹服务: 初始化");
        Self
    }
}

impl Default for FolderService {
    fn default() -> Self {
        Self::new()
    }
}

impl FolderService {
    // --- Folder Operations ---

    pub fn save_folder(&self, db: &Database, folder: Folder) -> Result<()> {
        info!(
            "数据库管理: 正在保存文件夹: '{}' (ID: {})",
            folder.name, folder.id
        );
        db.insert_folder(&folder)
            .inspect_err(|e| error!("数据库管理: 保存文件夹失败: {e}"))?;
        debug!("数据库管理: 文件夹保存成功: '{}'", folder.name);
        Ok(())
    }

    pub fn update_folder_name(&self, db: &Database, id: &str, name: String) -> Result<()> {
        info!("数据库管理: 正在更新文件夹名称 (ID: {id}, 新名称: {name})");
        db.update_folder_name(id, &name)
            .inspect_err(|e| error!("数据库管理: 更新文件夹名称失败: {e}"))?;
        debug!("数据库管理: 文件夹重命名成功 (ID: {id})");
        Ok(())
    }

    pub fn move_folder(&self, db: &Database, id: &str, parent_id: Option<String>) -> Result<()> {
        info!("数据库管理: 正在移动文件夹 (ID: {id}, 新父文件夹ID: {parent_id:?})");
        db.move_folder(id, parent_id)
            .inspect_err(|e| error!("数据库管理: 移动文件夹失败: {e}"))?;
        debug!("数据库管理: 文件夹移动成功 (ID: {id})");
        Ok(())
    }

    pub fn delete_folder(
        &self,
        db: &Database,
        notify: impl Fn(),
        id: &str,
        recursive: bool,
    ) -> Result<()> {
        info!("数据库管理: 正在删除文件夹 (ID: {id}, 递归: {recursive})");

        let result = if recursive {
            db.delete_folder_recursive(id)
        } else {
            db.delete_folder(id)
        };
        result.inspect_err(|e| error!("数据库管理: 删除文件夹失败: {e}"))?;

        // 同步内存数据
        debug!("数据库管理: 文件夹删除成功，刷新全量数据");
        notify();
        Ok(())
    }
}
