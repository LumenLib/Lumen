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
    match mode {
        "dark" => Theme::change(ThemeMode::Dark, None, cx),
        "system" => Theme::sync_system_appearance(None, cx),
        _ => Theme::change(ThemeMode::Light, None, cx),
    };

    // 2. 软件固定默认值：获取并应用基础配色
    let default_colors = theme_manager::ThemeColors::default();

    let mut theme = cx.global::<Theme>().clone();
    default_colors.apply_to_palette(&mut theme);
    cx.set_global(theme);

    // 3. 从样式方案加载自定义颜色和表面颜色
    let is_dark = cx.global::<Theme>().mode == ThemeMode::Dark;

    let surface = if style != "default" {
        let custom_scheme = {
            let loader = theme_manager::LOADER.read().ok();
            loader.and_then(|l| l.get_theme(style).cloned())
        };

        if let Some(scheme) = custom_scheme {
            let mut theme = cx.global::<Theme>().clone();

            if is_dark {
                scheme.dark.apply_to_palette(&mut theme);
            } else {
                scheme.light.apply_to_palette(&mut theme);
            }

            cx.set_global(theme);

            if is_dark {
                scheme.surface_dark.resolve(true)
            } else {
                scheme.surface_light.resolve(false)
            }
        } else {
            theme_manager::SurfaceColors::default().resolve(is_dark)
        }
    } else {
        theme_manager::SurfaceColors::default().resolve(is_dark)
    };

    if let Ok(mut s) = theme_manager::SURFACE.write() {
        *s = surface;
    }

    // 4. 应用缩放系数
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
