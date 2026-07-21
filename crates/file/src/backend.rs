use anyhow::Result;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

/// 远程文件条目
#[derive(Debug, Clone)]
pub struct RemoteFileEntry {
    pub name: String,
    /// 不透明版本标识。前端只做 == 比较：
    ///   同一内容 → 同一字符串；内容改变 → 字符串改变。
    pub version: String,
}

/// 附件同步后端接口
///
/// 每个实现（WebDAV、SFTP、Google Drive 等）
/// 负责将协议细节封闭在文件内部，对外仅暴露这 6 个操作。
pub trait AttachmentBackend: Send + Sync {
    /// 后端名称标识，如 "webdav"、"google_drive"
    fn name(&self) -> &str;

    /// 是否已启用
    fn is_enabled(&self) -> bool;

    /// 测试连接是否可用
    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;

    /// 上传文件，返回 version 标识
    fn upload(
        &self,
        local_path: PathBuf,
        name: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send>>;

    /// 下载文件，返回 version 标识
    fn download(
        &self,
        name: String,
        local_path: PathBuf,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send>>;

    /// 列举远程所有文件
    fn list(&self) -> Pin<Box<dyn Future<Output = Result<Vec<RemoteFileEntry>>> + Send>>;

    /// 删除远程文件
    fn delete(&self, name: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;

    /// 重命名远程文件
    fn rename(&self, old: String, new: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}
