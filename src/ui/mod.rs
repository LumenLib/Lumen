//! UI 模块
//!
//! 负责应用的所有用户界面组件，包括视图、组件和样式

use gpui::{App, px};
use gpui_component::{Theme, ThemeMode};
use log::info;

pub mod components;
pub mod icons;
pub mod theme_manager;
pub mod views;

/// 重新导出常用类型
pub use views::{LiteratureDetailView, LiteratureListView, MainWindow};

/// 应用全局主题
pub fn apply_theme(mode: &str, style: &str, scale: f32, cx: &mut App) {
    info!("UI: 正在应用主题 - 模式: {mode}, 样式: {style}, 缩放: {scale}");

    // 1. 先应用基础模式 (Light/Dark/System)
    let is_dark = match mode {
        "dark" => {
            Theme::change(ThemeMode::Dark, None, cx);
            true
        }
        "system" => {
            Theme::sync_system_appearance(None, cx);
            cx.global::<Theme>().mode == ThemeMode::Dark
        }
        _ => {
            Theme::change(ThemeMode::Light, None, cx);
            false
        }
    };

    // 2. 软件固定默认值：获取并应用 One 配色作为系统的基础/默认配色
    let default_colors = if is_dark {
        theme_manager::ThemeColors::one_dark()
    } else {
        theme_manager::ThemeColors::one_light()
    };

    let mut theme = cx.global::<Theme>().clone();
    default_colors.apply_to_palette(&mut theme);
    cx.set_global(theme);

    // 3. 如果指定了非默认样式方案，则在基础模式之上应用自定义颜色
    if style != "default" {
        let custom_scheme = {
            let loader = theme_manager::LOADER.read().ok();
            loader.and_then(|l| l.get_theme(style).cloned())
        };

        if let Some(scheme) = custom_scheme {
            // 获取当前（刚刚由 Theme::change 设置的）全局主题
            let mut theme = cx.global::<Theme>().clone();

            // 根据当前模式决定应用方案中的哪一部分颜色
            match theme.mode {
                ThemeMode::Dark => {
                    scheme.dark.apply_to_palette(&mut theme);
                }
                ThemeMode::Light => {
                    scheme.light.apply_to_palette(&mut theme);
                }
            };

            // 写回全局主题
            cx.set_global(theme);
        }
    }

    // 3. 应用缩放系数
    // 这一步非常重要，因为 Root 组件在 render 时会用 theme.font_size 覆盖 rem_size
    {
        let mut theme = cx.global::<Theme>().clone();
        theme.font_size = px(16.0 * scale);
        // 同时也按比例调整圆角，保持视觉协调
        theme.radius = px(6.0 * scale);
        theme.radius_lg = px(8.0 * scale);
        cx.set_global(theme);
    }
}
