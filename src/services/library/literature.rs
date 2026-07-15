use crate::RUNTIME;
use crate::services::analysis::fetch_rank;
use crate::services::{MainApp, filename};
use anyhow::Result;
use log::{debug, error, info, warn};
/// 数据库操作单例管理器
///
/// 负责协调持久化存储与内存数据的同步
use models::{Author, Literature};
use parser::normalize::sanitize_arxiv_identifiers;
use std::collections::HashSet;
use std::path::Path;
use std::{collections, fs};

pub struct LiteratureService;

impl LiteratureService {
    #[must_use]
    pub fn new() -> Self {
        debug!("文献服务: 初始化");
        Self
    }

    /// 寻找重复文献
    pub fn find_duplicates(&self, app: &MainApp) -> Vec<Vec<Literature>> {
        let literatures = app
            .db
            .get_all_literatures()
            .unwrap_or_default()
            .into_iter()
            .filter(|l| !l.folder_ids.iter().any(|s| s == "trash"))
            .collect::<Vec<_>>();

        debug!("查重: 扫描 {} 篇非回收站文献", literatures.len());

        if literatures.len() < 2 {
            debug!("查重: 文献数量不足，跳过");
            return Vec::new();
        }

        let mut groups: Vec<HashSet<String>> = Vec::new();

        // 1. 按 DOI 分组
        let mut doi_map: collections::HashMap<String, Vec<String>> = collections::HashMap::new();
        for lit in &literatures {
            if let Some(doi) = &lit.doi {
                let normalized_doi = doi.trim().to_lowercase();
                if !normalized_doi.is_empty() {
                    doi_map
                        .entry(normalized_doi)
                        .or_default()
                        .push(lit.id.clone());
                }
            }
        }
        let doi_dup_count = doi_map.values().filter(|ids| ids.len() > 1).count();
        for ids in doi_map.values() {
            if ids.len() > 1 {
                groups.push(ids.iter().cloned().collect());
            }
        }
        debug!("查重: DOI 分组发现 {} 组重复", doi_dup_count);

        // 2. 按 (标题 + 年份) 分组
        let mut title_year_map: collections::HashMap<(String, Option<i32>), Vec<String>> =
            collections::HashMap::new();
        for lit in &literatures {
            let normalized_title = lit
                .title
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>();
            if !normalized_title.is_empty() {
                title_year_map
                    .entry((normalized_title, lit.year))
                    .or_default()
                    .push(lit.id.clone());
            }
        }
        let ty_dup_count = title_year_map.values().filter(|ids| ids.len() > 1).count();
        for ids in title_year_map.values() {
            if ids.len() > 1 {
                groups.push(ids.iter().cloned().collect());
            }
        }
        debug!("查重: 标题+年份分组发现 {} 组重复", ty_dup_count);

        // 合并相交的组 (Union-Find 思想)
        let mut merged_groups: Vec<HashSet<String>> = Vec::new();
        for group in groups {
            let mut found = false;
            for merged in &mut merged_groups {
                if !merged.is_disjoint(&group) {
                    merged.extend(group.iter().cloned());
                    found = true;
                    break;
                }
            }
            if !found {
                merged_groups.push(group);
            }
        }
        debug!("查重: 初步合并后 {} 组", merged_groups.len());

        // 再次检查是否有可进一步合并的组
        let mut final_groups: Vec<HashSet<String>> = Vec::new();
        for mut group in merged_groups {
            let mut i = 0;
            while i < final_groups.len() {
                if final_groups[i].is_disjoint(&group) {
                    i += 1;
                } else {
                    let other = final_groups.remove(i);
                    group.extend(other);
                }
            }
            final_groups.push(group);
        }
        debug!("查重: 最终合并后 {} 组", final_groups.len());

        info!("查重完成: 发现 {} 组可能重复的文献", final_groups.len());

        final_groups
            .into_iter()
            .map(|ids| {
                ids.into_iter()
                    .filter_map(|id| literatures.iter().find(|l| l.id == id).cloned())
                    .collect()
            })
            .collect()
    }
}

impl Default for LiteratureService {
    fn default() -> Self {
        Self::new()
    }
}

impl LiteratureService {
    pub fn save_literature(&self, app: &MainApp, mut lit: Literature) -> Result<()> {
        sanitize_arxiv_identifiers(&mut lit);
        // 自动补充 CCF 分级信息 (如果缺失)
        let mut new_ccf_rank = None;
        let mut pub_name_for_update = None;

        if let Some(ref mut pub_info) = lit.publication
            && pub_info.ccf_rank.is_none()
            && let Some(rank) = app.ccf_service.get_rank(&pub_info.name)
        {
            info!("CCF: 自动识别文献 '{}' 的分级: {}", pub_info.name, rank);
            pub_info.ccf_rank = Some(rank.clone());
            new_ccf_rank = Some(rank);
            pub_name_for_update = Some(pub_info.name.clone());
        }

        info!(
            "数据库管理: 正在保存文献: '{}' (ID: {}, 附件数: {})",
            lit.title,
            lit.id,
            lit.attachments.len()
        );

        let was_new = app.db.get_literature(&lit.id)?.is_none();
        debug!(
            "保存文献: '{}' 为{}记录",
            lit.title,
            if was_new { "新" } else { "已有" }
        );
        let old_pub_name = if !was_new {
            app.db
                .get_literature(&lit.id)?
                .and_then(|l| l.publication.map(|p| p.name))
        } else {
            None
        };

        app.db
            .insert_literature(&lit)
            .inspect_err(|e| error!("数据库管理: 保存文献到数据库失败: {e}"))?;

        // --- 批量更新同名期刊的 CCF 分级 ---
        if let (Some(rank), Some(pub_name)) = (new_ccf_rank, pub_name_for_update) {
            let all_lits = app.db.get_all_literatures()?;
            let mut updates = Vec::new();
            for mut other_lit in all_lits {
                if other_lit.id == lit.id {
                    continue;
                }
                if let Some(ref mut other_pub) = other_lit.publication
                    && other_pub.name == pub_name
                    && other_pub.ccf_rank.as_ref() != Some(&rank)
                {
                    other_pub.ccf_rank = Some(rank.clone());
                    updates.push(other_lit);
                }
            }
            if !updates.is_empty() {
                info!(
                    "CCF: 检测到同名期刊 '{pub_name}'，正在批量更新 {} 篇文献的 CCF 分级为 '{rank}'",
                    updates.len()
                );
                for updated_lit in updates {
                    if let Err(e) = app.db.insert_literature(&updated_lit) {
                        error!("CCF: 批量更新文献[{}]失败: {}", updated_lit.id, e);
                    }
                }
            }
        }

        // 重新从数据库加载以获取规范化后的作者 ID 等信息
        let reloaded_lit = app
            .db
            .get_literature(&lit.id)?
            .ok_or_else(|| anyhow::anyhow!("保存后无法重新获取文献记录 (ID: {})", lit.id))?;

        let is_new_entry = was_new;
        let name_changed = if !is_new_entry {
            let new_name = reloaded_lit.publication.as_ref().map(|p| p.name.as_str());
            old_pub_name.as_deref() != new_name
        } else {
            false
        };
        if name_changed {
            info!("文献服务: 检测到刊名变化，将触发分级查询");
        }

        let lit_id_clone = reloaded_lit.id.clone();
        let trigger_title = reloaded_lit.publication.as_ref().map(|p| p.name.clone());

        // 触发 EasyScholar 查询 (针对新文献 OR 刊名发生变化的文献)
        if (is_new_entry || name_changed)
            && let Ok(Some(key)) = app.db.get_sync_meta("easyscholar_key")
            && let Some(title) = trigger_title
            && !title.is_empty()
        {
            info!("EasyScholar: 触发后台排名查询 ({title})");
            debug!("EasyScholar: 后台任务已提交");
            let db = app.db.clone();
            let refresh_tx = app.refresh_tx.lock().unwrap().clone();

            RUNTIME.spawn(async move {
                match fetch_rank(&title, &key).await {
                    Ok(rank) => {
                        info!("EasyScholar: 查询成功 {title:?} -> {rank:?}");

                        // 从数据库获取并更新文献排名
                        if let Ok(Some(mut lit)) = db.get_literature(&lit_id_clone) {
                            if let Some(pub_info) = &mut lit.publication {
                                pub_info.jcr_rank = rank.jcr.clone();
                                pub_info.cas_rank = rank.cas.clone();
                            }
                            lit.version += 1;
                            lit.is_dirty = true;
                            lit.updated_at =
                                chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                            let _ = db._update_literature_row(&lit);

                            if let Some(tx) = &refresh_tx {
                                let _ =
                                    tx.send(crate::services::data_store::RefreshMsg::DataChanged);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("EasyScholar: 查询失败 [{title}]: {e}");
                    }
                }
            });
        }

        debug!("保存文献流程完成 (ID: {})", lit.id);
        Ok(())
    }

    /// 更新文献详细信息（包含智能重命名和云端清理逻辑）
    pub fn update_literature_details(&self, app: &MainApp, mut lit: Literature) -> Result<()> {
        sanitize_arxiv_identifiers(&mut lit);
        let lit_id = lit.id.clone();
        info!("更新文献[{lit_id}]: {}", lit.title);

        // --- 智能重命名逻辑 ---
        let template = {
            let config = app.config.lock().expect("Failed to lock AppConfig");
            config.filename_template.clone()
        };

        // 预先提取元数据
        let (last_name, first_name) = lit.authors.first().map_or_else(
            || ("Unknown".to_string(), String::new()),
            |a| (a.last_name.clone(), a.first_name.clone()),
        );
        let year_str = lit
            .year
            .map_or_else(|| "0000".to_string(), |y| y.to_string());
        let publication = lit
            .publication
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let lit_title_clone = lit.title.clone();

        let att_count = lit.attachments.len();
        debug!("更新文献详情: {} 个附件待处理", att_count);
        for att in lit.attachments.iter_mut() {
            let path = Path::new(&att.file_path);
            if !path.exists() {
                continue;
            }

            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if extension.is_empty() {
                continue;
            }

            let options = filename::FilenameOptions::new(
                &last_name,
                &first_name,
                &year_str,
                &lit_title_clone,
                &publication,
                extension,
                att.is_main,
            );
            let new_filename = filename::generate_literature_filename(&options, Some(&template));

            if new_filename != att.file_name {
                let parent = path.parent().unwrap_or_else(|| Path::new("."));
                let new_path = parent.join(&new_filename);
                let old_filename = att.file_name.clone();

                info!("自动重命名: {old_filename} -> {new_filename}");

                if let Err(e) = fs::rename(path, &new_path) {
                    error!("重命名失败 {path:?} -> {new_path:?}: {e}");
                    continue;
                }

                app.sync_service.queue_remote_rename(&att.id, &old_filename);

                att.file_name = new_filename;
                att.file_path = new_path.to_string_lossy().to_string();

                att.is_dirty = true;
            }
        }

        // 更新版本和时间
        lit.version += 1;
        lit.updated_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        self.save_literature(app, lit)?;

        info!("文献详细信息更新流程完成 (ID: {lit_id})");
        Ok(())
    }

    pub fn update_literature_reading_status(
        &self,
        app: &MainApp,
        id: &str,
        status: models::ReadingStatus,
    ) -> Result<()> {
        info!("数据库管理: 正在更新文献阅读状态 (ID: {id}, status: {status:?})");
        app.db
            .update_reading_status(id, status)
            .inspect_err(|e| error!("数据库管理: 更新阅读状态失败: {e}"))?;
        Ok(())
    }

    pub fn delete_literature(&self, app: &MainApp, id: &str) -> Result<()> {
        info!("数据库管理: 准备删除文献 (ID: {id})");

        // 1. 获取文献附件并标记为删除以便同步删除
        if let Ok(Some(lit)) = app.db.get_literature(id) {
            info!(
                "数据库管理: 正在清理文献 '{}' 的 {} 个附件",
                lit.title,
                lit.attachments.len()
            );
            for att in &lit.attachments {
                debug!(
                    "数据库管理: 将附件[{}]标记为删除以便同步删除: {}",
                    att.id, att.file_path
                );
                if let Err(e) = app.db.delete_attachment(&att.id) {
                    warn!("数据库管理: 标记附件为删除失败 [{}]: {}", att.id, e);
                }
                if let Err(e) = app.file_manager.trash_file(&att.file_path) {
                    warn!("文件系统: 移入回收站失败 [{}]: {e}", att.file_path);
                }
            }
        } else {
            warn!("数据库管理: 未找到待删除文献 (ID: {id})");
        }

        // 2. 删除数据库记录
        app.db
            .delete_literature(id)
            .inspect_err(|e| error!("数据库管理: 删除数据库记录失败: {e}"))?;
        info!("数据库管理: 数据库记录已删除 (ID: {id})");
        Ok(())
    }

    // --- Relationship Operations ---

    pub fn set_literature_authors(
        &self,
        app: &MainApp,
        literature_id: &str,
        authors: Vec<Author>,
    ) -> Result<()> {
        info!(
            "数据库管理: 正在更新文献作者关联 (文献ID: {}, 作者数: {})",
            literature_id,
            authors.len()
        );
        app.db
            .set_literature_authors(literature_id, &authors)
            .inspect_err(|e| error!("数据库管理: 更新文献作者关联失败: {e}"))?;
        Ok(())
    }

    pub fn add_literature_to_folder(
        &self,
        app: &MainApp,
        literature_id: &str,
        folder_id: &str,
    ) -> Result<()> {
        info!("数据库: 开始添加文献[{literature_id}]到文件夹[{folder_id}]");
        let mut folder_ids = app
            .db
            .get_folders_for_literature(literature_id)
            .unwrap_or_default();

        if folder_ids.iter().any(|s| s == folder_id) {
            info!("数据库: 文献[{literature_id}]已在文件夹[{folder_id}]中，无需重复添加");
        } else {
            folder_ids.push(folder_id.to_string());
            app.db
                .set_literature_folders(literature_id, &folder_ids)
                .inspect_err(|e| error!("数据库: 保存文件夹关系失败: {e}"))?;
            info!("数据库: 文件夹关系已保存到数据库");
        }
        Ok(())
    }

    pub fn remove_literature_from_folder(
        &self,
        app: &MainApp,
        literature_id: &str,
        folder_id: &str,
    ) -> Result<()> {
        info!("数据库: 开始从文件夹[{folder_id}]移除文献[{literature_id}]");
        let mut folder_ids = app
            .db
            .get_folders_for_literature(literature_id)
            .unwrap_or_default();

        if folder_ids.iter().any(|s| s == folder_id) {
            folder_ids.retain(|id| id != folder_id);
            app.db
                .set_literature_folders(literature_id, &folder_ids)
                .inspect_err(|e| error!("数据库: 移除文件夹关系失败: {e}"))?;
            info!("数据库: 文件夹关系已从数据库移除");
        } else {
            info!("数据库: 文献[{literature_id}]不在文件夹[{folder_id}]中，无需移除");
        }
        Ok(())
    }

    pub fn set_literature_folders(
        &self,
        app: &MainApp,
        literature_id: &str,
        folder_ids: Vec<String>,
    ) -> Result<()> {
        info!(
            "数据库管理: 正在更新文献文件夹关联 (文献ID: {}, 文件夹数: {})",
            literature_id,
            folder_ids.len()
        );
        app.db
            .set_literature_folders(literature_id, &folder_ids)
            .inspect_err(|e| error!("数据库管理: 保存文献文件夹关联失败: {e}"))?;
        Ok(())
    }

    pub fn set_literature_tags(
        &self,
        app: &MainApp,
        literature_id: &str,
        tags: Vec<String>,
    ) -> Result<()> {
        info!(
            "数据库管理: 正在更新文献标签关联 (文献ID: {}, 标签数: {})",
            literature_id,
            tags.len()
        );
        app.db
            .set_literature_tags(literature_id, &tags)
            .inspect_err(|e| error!("数据库管理: 保存文献标签关联失败: {e}"))?;
        app.notify_data_changed();
        Ok(())
    }
}
