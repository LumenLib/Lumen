use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
#[exclude = "styles/*"]
#[exclude = "csl/*"]
struct EmbedAssets;

pub struct Assets;

impl Assets {
    /// 直接获取嵌入式文件内容 (用于非 GPUI 上下文，如数据加载)
    #[must_use]
    pub fn get(file_path: &str) -> Option<rust_embed::EmbeddedFile> {
        EmbedAssets::get(file_path)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        // 首先尝试从我们的自定义嵌入资产中加载
        if let Some(file) = EmbedAssets::get(path) {
            return Ok(Some(file.data));
        }

        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let files = EmbedAssets::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.to_string()))
            .collect::<Vec<_>>();

        Ok(files)
    }
}
