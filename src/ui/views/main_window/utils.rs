use gpui::prelude::*;
use gpui::{div, rems};
use gpui_component::Theme;
use models::Literature;

/// 提取 `ArXiv` ID
/// `注意：sanitize_arxiv_identifiers()` 已经在解析时规范化了 `arxiv_id（移除前缀、提取自` DOI/URL）
pub fn extract_arxiv_id(lit: &Literature) -> Option<String> {
    lit.arxiv_id
        .as_ref()
        .filter(|id| !id.trim().is_empty())
        .map(|id| id.trim().to_string())
}

/// 跨平台打开 URL
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("cmd")
            .arg("/c")
            .arg("start")
            .arg("")
            .arg(url)
            .creation_flags(0x08000000)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// 渲染分隔线（folder_selector 等组件使用）
#[must_use]
pub fn render_separator(theme: &Theme) -> impl IntoElement {
    div().h(rems(0.0625)).bg(theme.border).my_1()
}
