use gpui::prelude::*;
use gpui::{Pixels, div, px, rems};
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

/// 判断菜单是否应该向上弹出
pub fn is_menu_upward(y: Pixels, window_height: Pixels) -> bool {
    y + px(300.0) > window_height
}

/// 计算获取元数据子菜单的 Y 轴偏移量
pub fn calculate_fetch_submenu_y_offset(is_upward: bool) -> Pixels {
    // 估计单选时的总高度约为 156px, "从...获取" 项在 100px 处
    if is_upward {
        px(100.0) - px(156.0)
    } else {
        px(100.0)
    }
}

/// 计算文件夹选择器子菜单的 Y 轴偏移量
pub fn calculate_folder_submenu_y_offset(selected_count: usize, is_upward: bool) -> Pixels {
    if selected_count <= 1 {
        // 单选：估计总高度 156px，项在 132px 处
        if is_upward {
            px(132.0) - px(156.0)
        } else {
            px(132.0)
        }
    } else {
        // 多选：估计总高度 100px，项在 68px 处
        if is_upward {
            px(68.0) - px(100.0)
        } else {
            px(68.0)
        }
    }
}

/// 渲染分隔线
#[must_use]
pub fn render_separator(theme: &Theme) -> impl IntoElement {
    div().h(rems(0.0625)).bg(theme.border).my_1()
}
