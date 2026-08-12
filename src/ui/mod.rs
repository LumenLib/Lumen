//! UI 模块
//!
//! 负责应用的所有用户界面组件，包括视图、组件和样式

use gpui::{App, px};
use gpui_component::{Theme, ThemeMode};
use log::info;

pub mod actions;
pub mod components;
pub mod dialogs;
pub mod notification;
pub mod views;

/// 重新导出常用类型
pub use views::{LiteratureDetailView, LiteratureListView, MainWindow};

/// 应用全局主题
pub fn apply_theme(mode: &str, style: &str, scale: f32, cx: &mut App) {
    info!("UI: 正在应用主题 - 模式: {mode}, 样式: {style}, 缩放: {scale}");

    // 1. 应用基础模式 (Light/Dark/System)
    match mode {
        "dark" => Theme::change(ThemeMode::Dark, None, cx),
        "system" => Theme::sync_system_appearance(None, cx),
        _ => Theme::change(ThemeMode::Light, None, cx),
    };

    let is_dark = cx.global::<Theme>().mode == ThemeMode::Dark;
    let mut theme = cx.global::<Theme>().clone();

    // 2. 应用集中管理的默认基础配色
    let default_colors = if is_dark {
        models::theme::ThemeColors::dark_default()
    } else {
        models::theme::ThemeColors::light_default()
    };
    crate::app_state::theme::apply_colors_to_theme(&default_colors, &mut theme);

    // 3. 从样式方案加载自定义颜色和表面颜色
    let custom_scheme = if style != "default" {
        let loader = crate::app_state::theme::ThemeLoaderState::read(cx);
        loader.get_theme(style).cloned()
    } else {
        None
    };

    let surface = if let Some(scheme) = custom_scheme {
        let colors = if is_dark { &scheme.dark } else { &scheme.light };
        crate::app_state::theme::apply_colors_to_theme(colors, &mut theme);

        let surface_colors = if is_dark {
            &scheme.surface_dark
        } else {
            &scheme.surface_light
        };
        crate::app_state::theme::resolve_surface(surface_colors, is_dark)
    } else {
        crate::app_state::theme::resolve_surface(&models::theme::SurfaceColors::default(), is_dark)
    };

    crate::app_state::theme::SurfaceState::set(surface, cx);

    // 4. 应用缩放系数（字体大小与圆角）
    theme.font_size = px(16.0 * scale);
    theme.radius = px(6.0 * scale);
    theme.radius_lg = px(8.0 * scale);

    // 5. 统一提交更新后的全局主题对象
    cx.set_global(theme);
}
