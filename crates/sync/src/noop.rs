use crate::backend::{AttachmentBackend, RemoteFileEntry};
use anyhow::{Result, anyhow};
use log::warn;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

/// 空操作后端 — 未启用任何远程后端时使用
pub struct NoopBackend;

impl AttachmentBackend for NoopBackend {
    fn name(&self) -> &str {
        "noop"
    }

    fn is_enabled(&self) -> bool {
        false
    }

    fn test_connection(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async {
            warn!("NoopBackend: test_connection — 未配置文件同步后端");
            Err(anyhow!("未配置文件同步后端"))
        })
    }

    fn upload(
        &self,
        _local_path: PathBuf,
        _name: String,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send>> {
        Box::pin(async {
            warn!("NoopBackend: upload — 未配置文件同步后端");
            Err(anyhow!("未配置文件同步后端"))
        })
    }

    fn download(
        &self,
        _name: String,
        _local_path: PathBuf,
    ) -> Pin<Box<dyn Future<Output = Result<Option<String>>> + Send>> {
        Box::pin(async {
            warn!("NoopBackend: download — 未配置文件同步后端");
            Err(anyhow!("未配置文件同步后端"))
        })
    }

    fn list(&self) -> Pin<Box<dyn Future<Output = Result<Vec<RemoteFileEntry>>> + Send>> {
        Box::pin(async {
            warn!("NoopBackend: list — 未配置文件同步后端，返回空列表");
            Ok(Vec::new())
        })
    }

    fn delete(&self, _name: String) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async {
            warn!("NoopBackend: delete — 未配置文件同步后端");
            Ok(())
        })
    }

    fn rename(
        &self,
        _old: String,
        _new: String,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async {
            warn!("NoopBackend: rename — 未配置文件同步后端");
            Ok(())
        })
    }
}
