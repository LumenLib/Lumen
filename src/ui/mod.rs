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

    // 1. 先应用基础模式 (Light/Dark/System)
    match mode {
        "dark" => Theme::change(ThemeMode::Dark, None, cx),
        "system" => Theme::sync_system_appearance(None, cx),
        _ => Theme::change(ThemeMode::Light, None, cx),
    };

    // 2. 软件固定默认值：获取并应用基础配色
    let default_colors = models::theme::ThemeColors::default();

    let mut theme = cx.global::<Theme>().clone();
    // 内置默认：弹出菜单背景与主背景有层次区分，避免纯黑白
    let is_dark = theme.mode == ThemeMode::Dark;
    if is_dark {
        theme.popover = crate::app_state::theme::parse_color("#262626"); // neutral-800
    } else {
        theme.popover = crate::app_state::theme::parse_color("#fafafa"); // neutral-50
    }
    crate::app_state::theme::apply_colors_to_theme(&default_colors, &mut theme);
    cx.set_global(theme);

    // 3. 从样式方案加载自定义颜色和表面颜色
    let is_dark = cx.global::<Theme>().mode == ThemeMode::Dark;

    let surface = if style != "default" {
        let custom_scheme = {
            let loader = crate::app_state::theme::ThemeLoaderState::read(cx);
            loader.get_theme(style).cloned()
        };

        if let Some(scheme) = custom_scheme {
            let mut theme = cx.global::<Theme>().clone();

            if is_dark {
                crate::app_state::theme::apply_colors_to_theme(&scheme.dark, &mut theme);
            } else {
                crate::app_state::theme::apply_colors_to_theme(&scheme.light, &mut theme);
            }

            cx.set_global(theme);

            if is_dark {
                crate::app_state::theme::resolve_surface(&scheme.surface_dark, true)
            } else {
                crate::app_state::theme::resolve_surface(&scheme.surface_light, false)
            }
        } else {
            crate::app_state::theme::resolve_surface(
                &models::theme::SurfaceColors::default(),
                is_dark,
            )
        }
    } else {
        crate::app_state::theme::resolve_surface(&models::theme::SurfaceColors::default(), is_dark)
    };

    crate::app_state::theme::SurfaceState::set(surface, cx);

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
