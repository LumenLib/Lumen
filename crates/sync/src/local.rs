use log::{debug, info, warn};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// 上传操作结果
#[derive(Debug, Clone)]
pub struct FileUploadResult {
    pub final_path: PathBuf,
    pub final_name: String,
    pub size: u64,
}

/// 本地文件管理器，负责文献附件的复制、移动、删除、回收站
#[derive(Debug, Clone)]
pub struct LocalFileManager {
    attachments_dir: PathBuf,
}

impl LocalFileManager {
    pub fn new<P: AsRef<Path>>(attachments_dir: P) -> Result<Self, io::Error> {
        let attachments_dir = attachments_dir.as_ref().to_path_buf();
        info!("文件系统: 初始化附件目录: {}", attachments_dir.display());

        if !attachments_dir.exists() {
            debug!("文件系统: 正在创建附件目录...");
            fs::create_dir_all(&attachments_dir)?;
        }

        Ok(Self { attachments_dir })
    }

    pub fn upload_file_with_name(
        &self,
        source_path: &Path,
        target_name: &str,
    ) -> Result<FileUploadResult, io::Error> {
        debug!("文件系统: 正在上传文件: {target_name}");
        let target_dir = &self.attachments_dir;

        let target_path = Path::new(target_name);
        let stem = target_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let extension = target_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| format!(".{e}"))
            .unwrap_or_default();
        let parent_dir = target_path.parent().unwrap_or(Path::new(""));

        let mut final_name = target_name.to_string();
        let mut dest_path = target_dir.join(&final_name);
        let mut counter = 1;

        while dest_path.exists() {
            final_name = format!(
                "{}/{}({}){}",
                parent_dir.display(),
                stem,
                counter,
                extension
            )
            .trim_start_matches('/')
            .to_string();
            dest_path = target_dir.join(&final_name);
            counter += 1;
        }

        if counter > 1 {
            info!("文件系统: 文件名冲突，已重命名为: {final_name}");
        }

        if let Some(parent) = dest_path.parent()
            && !parent.exists()
        {
            debug!("文件系统: 正在创建目录: {}", parent.display());
            fs::create_dir_all(parent)?;
        }

        debug!("文件系统: 正在拷贝文件到: {}", dest_path.display());
        fs::copy(source_path, &dest_path)?;

        let metadata = fs::metadata(&dest_path)?;
        info!(
            "文件系统: 成功上传文件 '{}' (大小: {} bytes)",
            final_name,
            metadata.len()
        );

        Ok(FileUploadResult {
            final_path: dest_path,
            final_name,
            size: metadata.len(),
        })
    }

    pub fn get_attachments_dir(&self) -> PathBuf {
        self.attachments_dir.clone()
    }

    pub fn trash_file(&self, file_path: &str) -> Result<(), io::Error> {
        let path = Path::new(file_path);
        if !path.exists() {
            debug!("文件系统: 文件不存在，跳过移入回收站: {file_path}");
            return Ok(());
        }

        info!("文件系统: 正在将文件移入回收站: {file_path}");
        trash::delete(path).map_err(|e| {
            warn!("文件系统: 移入回收站失败 [{}]: {e}", file_path);
            io::Error::new(io::ErrorKind::Other, e)
        })
    }

    pub fn trash_all(&self) -> Result<(), io::Error> {
        info!(
            "文件系统: 正在将整个附件目录移入回收站: {}",
            self.attachments_dir.display()
        );
        if self.attachments_dir.exists() {
            for entry in fs::read_dir(&self.attachments_dir)? {
                let entry = entry?;
                if let Err(e) = trash::delete(entry.path()) {
                    warn!("文件系统: 移入回收站失败 [{}]: {e}", entry.path().display());
                }
            }
            debug!("文件系统: 重新创建空的附件目录...");
            fs::create_dir_all(&self.attachments_dir)?;
        }
        Ok(())
    }
}
