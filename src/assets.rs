use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets"]
#[exclude = "styles/*"]
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
        // 1) 优先查 Lumen 自有 assets
        if let Some(file) = EmbedAssets::get(path) {
            return Ok(Some(file.data));
        }
        // 2) 回退到 gpui-component 内置图标
        if let Ok(Some(data)) = gpui_component_assets::Assets::new("").load(path) {
            return Ok(Some(data));
        }
        Ok(None)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut files: Vec<SharedString> = EmbedAssets::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| p.into())
            .collect();
        if let Ok(mut more) = gpui_component_assets::Assets::new("").list(path) {
            files.append(&mut more);
        }
        Ok(files)
    }
}
