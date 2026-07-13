use crate::services::MainApp;
use anyhow::Result;
use log::{debug, error, info};

/// 数据库操作单例管理器
///
/// 负责协调持久化存储与内存数据的同步
pub struct TagService;

impl TagService {
    #[must_use]
    pub fn new() -> Self {
        debug!("标签服务: 初始化");
        Self
    }
}

impl Default for TagService {
    fn default() -> Self {
        Self::new()
    }
}

impl TagService {
    // --- Tag Operations ---

    pub fn get_all_tags_with_counts(&self, app: &MainApp) -> Result<Vec<(models::Tag, usize)>> {
        let db = &app.db;
        Ok(db.get_all_tags_with_counts()?)
    }

    pub fn update_tag(&self, app: &MainApp, id: &str, name: &str, color: &str) -> Result<()> {
        info!("数据库管理: 准备更新标签 (ID: {id}), 新名称: {name}, 新颜色: {color}");
        app.db
            .update_tag_name(id, name)
            .inspect_err(|e| error!("数据库管理: 更新标签名称失败: {e}"))?;
        app.db
            .update_tag_color(id, color)
            .inspect_err(|e| error!("数据库管理: 更新标签颜色失败: {e}"))?;
        app.notify_data_changed();
        Ok(())
    }

    pub fn delete_tag(&self, app: &MainApp, id: &str) -> Result<()> {
        info!("数据库管理: 准备删除标签 (ID: {id})");
        app.db
            .delete_tag(id)
            .inspect_err(|e| error!("数据库管理: 删除标签失败: {e}"))?;
        app.notify_data_changed();
        info!("数据库管理: 标签已从数据库删除");
        Ok(())
    }

    // --- Relationship Operations ---

    pub fn add_tag_to_literature(&self, app: &MainApp, lit_id: &str, tag_name: &str) -> Result<()> {
        info!("数据库管理: 为文献添加标签 (文献ID: {lit_id}, 标签名: {tag_name})");
        app.db
            .add_tag_to_literature(lit_id, tag_name)
            .inspect_err(|e| error!("数据库管理: 添加文献标签失败: {e}"))?;
        app.notify_data_changed();
        Ok(())
    }

    pub fn remove_tag_from_literature(
        &self,
        app: &MainApp,
        lit_id: &str,
        tag_name: &str,
    ) -> Result<()> {
        info!("数据库管理: 从文献移除标签 (文献ID: {lit_id}, 标签名: {tag_name})");
        app.db
            .remove_tag_from_literature(lit_id, tag_name)
            .inspect_err(|e| error!("数据库管理: 移除文献标签失败: {e}"))?;
        app.notify_data_changed();
        Ok(())
    }
}
