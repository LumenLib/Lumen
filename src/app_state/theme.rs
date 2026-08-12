//! 主题运行时态（GPUI 耦合）
//!
//! 仅收纳与 GPUI 强耦合的主题运行时部分：
//! - `SurfaceState` / `ThemeLoaderState`：主题运行时态（注册为 GPUI Global）
//! - `surface()` / `parse_color()` 访问器
//! - 主题类型 → gpui_component::Theme 的应用（`apply_colors_to_theme`）
//! - 表面色解析（`resolve_surface`）
//! - `ThemeSelectItem`（gpui_component 下拉项）
//!
//! 纯数据见 `models::theme`；磁盘加载见 `services::theme::ThemeLoader`。

use gpui::{App, Global, Hsla, SharedString};
use gpui_component::Theme;
use gpui_component::select::SelectItem;

use models::theme::{ResolvedSurface, SurfaceColors, ThemeColors};
use services::theme::ThemeLoader;

/// 解析后的表面色运行时态，注册为 GPUI Global。
///
/// 与 `ConfigStore` / `UiState` 同属 GPUI 进程级状态，统一通过
/// `cx.global::<SurfaceState>()` / `cx.set_global(...)` 访问，
/// 不再使用裸 `LazyLock<RwLock<>>` 进程单例。
pub struct SurfaceState {
    pub inner: ResolvedSurface,
}

impl Global for SurfaceState {}

impl SurfaceState {
    /// 注册默认（透明）表面色 Global，应在 app 启动时调用一次。
    pub fn init(cx: &mut App) {
        cx.set_global(Self {
            inner: ResolvedSurface::default(),
        });
    }

    /// 读取当前表面色（clone 一份，避免持有全局锁）。
    pub fn read(cx: &App) -> ResolvedSurface {
        cx.global::<Self>().inner.clone()
    }

    /// 更新表面色并刷新所有窗口。
    pub fn set(surface: ResolvedSurface, cx: &mut App) {
        cx.set_global(Self { inner: surface });
        cx.refresh_windows();
    }
}

/// 读取当前表面色（clone）。
///
/// 用法：在 render 内先 `let surface = surface(cx);`，随后 `surface.hover_bg`。
pub fn surface(cx: &App) -> ResolvedSurface {
    SurfaceState::read(cx)
}

pub fn parse_color(hex: &str) -> Hsla {
    gpui_component::try_parse_color(hex).unwrap_or(gpui::black())
}

/// 已加载主题缓存，注册为 GPUI Global。
///
/// 与 `SurfaceState` 一样走 GPUI 状态系统；热重载时整体替换并刷新窗口。
pub struct ThemeLoaderState {
    pub loader: ThemeLoader,
}

impl Global for ThemeLoaderState {}

impl ThemeLoaderState {
    /// 读取当前主题缓存（clone）。
    pub fn read(cx: &App) -> ThemeLoader {
        cx.global::<Self>().loader.clone()
    }

    /// 替换整个主题缓存并刷新所有窗口。
    pub fn set(loader: ThemeLoader, cx: &mut App) {
        cx.set_global(Self { loader });
        cx.refresh_windows();
    }
}

struct SurfaceDefaults {
    selected_text: &'static str,
    selected_faint: &'static str,
    selected_hover: &'static str,
    hover_bg: &'static str,
    hover_highlight: &'static str,
    hover_folder: &'static str,
    danger_hover: &'static str,
    danger_ghost: &'static str,
    window_button_hover: &'static str,
    card_bg: &'static str,
    section_bg: &'static str,
    info_bg: &'static str,
    border_faint: &'static str,
    chip_bg: &'static str,
    sidebar_tab_inactive: &'static str,
    drop_overlay: &'static str,
    label_disabled: &'static str,
}

const LIGHT_SURFACE: SurfaceDefaults = SurfaceDefaults {
    selected_text: "#4078f2cc",
    selected_faint: "#4078f20d",
    selected_hover: "#4078f226",
    hover_bg: "#e5e5e619",
    hover_highlight: "#e5e5e680",
    hover_folder: "#f0f0f080",
    danger_hover: "#e45649e6",
    danger_ghost: "#e456494d",
    window_button_hover: "#a0a1a733",
    card_bg: "#f0f0f026",
    section_bg: "#f0f0f033",
    info_bg: "#f0f0f04d",
    border_faint: "#e5e5e680",
    chip_bg: "#383a4280",
    sidebar_tab_inactive: "#383a42cc",
    drop_overlay: "#fafafae6",
    label_disabled: "#a0a1a780",
};

const DARK_SURFACE: SurfaceDefaults = SurfaceDefaults {
    selected_text: "#61afefcc",
    selected_faint: "#61afef0d",
    selected_hover: "#61afef26",
    hover_bg: "#3e445219",
    hover_highlight: "#3e445280",
    hover_folder: "#2c313a80",
    danger_hover: "#e06c75e6",
    danger_ghost: "#e06c754d",
    window_button_hover: "#5c637033",
    card_bg: "#2c313a26",
    section_bg: "#2c313a33",
    info_bg: "#2c313a4d",
    border_faint: "#3e445280",
    chip_bg: "#abb2bf80",
    sidebar_tab_inactive: "#abb2bfcc",
    drop_overlay: "#282c34e6",
    label_disabled: "#5c637080",
};

/// 把表面色方案解析为最终色值（带浅/深模式回退）。
pub fn resolve_surface(colors: &SurfaceColors, is_dark: bool) -> ResolvedSurface {
    let def = if is_dark {
        &DARK_SURFACE
    } else {
        &LIGHT_SURFACE
    };
    fn resolve_field(v: &Option<String>, fallback: &str) -> Hsla {
        v.as_deref()
            .map(parse_color)
            .unwrap_or_else(|| parse_color(fallback))
    }
    ResolvedSurface {
        selected_text: resolve_field(&colors.selected_text, def.selected_text),
        selected_faint: resolve_field(&colors.selected_faint, def.selected_faint),
        selected_hover: resolve_field(&colors.selected_hover, def.selected_hover),
        hover_bg: resolve_field(&colors.hover_bg, def.hover_bg),
        hover_highlight: resolve_field(&colors.hover_highlight, def.hover_highlight),
        hover_folder: resolve_field(&colors.hover_folder, def.hover_folder),
        danger_hover: resolve_field(&colors.danger_hover, def.danger_hover),
        danger_ghost: resolve_field(&colors.danger_ghost, def.danger_ghost),
        window_button_hover: resolve_field(&colors.window_button_hover, def.window_button_hover),
        card_bg: resolve_field(&colors.card_bg, def.card_bg),
        section_bg: resolve_field(&colors.section_bg, def.section_bg),
        info_bg: resolve_field(&colors.info_bg, def.info_bg),
        border_faint: resolve_field(&colors.border_faint, def.border_faint),
        chip_bg: resolve_field(&colors.chip_bg, def.chip_bg),
        sidebar_tab_inactive: resolve_field(&colors.sidebar_tab_inactive, def.sidebar_tab_inactive),
        drop_overlay: resolve_field(&colors.drop_overlay, def.drop_overlay),
        label_disabled: resolve_field(&colors.label_disabled, def.label_disabled),
    }
}

/// 把基础颜色方案应用到 gpui-component 主题调色板。
pub fn apply_colors_to_theme(colors: &ThemeColors, theme: &mut Theme) {
    if let Some(c) = &colors.foreground {
        theme.foreground = parse_color(c);
    }
    if let Some(c) = &colors.muted_foreground {
        theme.muted_foreground = parse_color(c);
    }
    if let Some(c) = &colors.popover_foreground {
        theme.popover_foreground = parse_color(c);
    }
    if let Some(c) = &colors.link {
        theme.link = parse_color(c);
    }
    if let Some(c) = &colors.background {
        theme.background = parse_color(c);
    }
    if let Some(c) = &colors.title_bar {
        theme.title_bar = parse_color(c);
    }
    if let Some(c) = &colors.title_bar_border {
        theme.title_bar_border = parse_color(c);
    }
    if let Some(c) = &colors.sidebar {
        theme.sidebar = parse_color(c);
    }
    if let Some(c) = &colors.sidebar_accent {
        theme.sidebar_accent = parse_color(c);
    }
    if let Some(c) = &colors.muted {
        theme.muted = parse_color(c);
    }
    if let Some(c) = &colors.accent {
        theme.accent = parse_color(c);
    }
    if let Some(c) = &colors.primary {
        theme.primary = parse_color(c);
    }
    if let Some(c) = &colors.secondary {
        theme.secondary = parse_color(c);
    }
    if let Some(c) = &colors.danger {
        theme.danger = parse_color(c);
    }
    if let Some(c) = &colors.success {
        theme.success = parse_color(c);
    }
    if let Some(c) = &colors.warning {
        theme.warning = parse_color(c);
    }
    if let Some(c) = &colors.info {
        theme.info = parse_color(c);
    }
    if let Some(c) = &colors.popover {
        theme.popover = parse_color(c);
    }
    if let Some(c) = &colors.status_bar {
        theme.status_bar = parse_color(c);
    }
    if let Some(c) = &colors.status_bar_border {
        theme.status_bar_border = parse_color(c);
    }
    if let Some(c) = &colors.skeleton {
        theme.skeleton = parse_color(c);
    }
    if let Some(c) = &colors.progress_bar {
        theme.progress_bar = parse_color(c);
    }
    if let Some(c) = &colors.overlay {
        theme.overlay = parse_color(c);
    }
    if let Some(c) = &colors.accent_foreground {
        theme.accent_foreground = parse_color(c);
    }
    if let Some(c) = &colors.primary_foreground {
        theme.primary_foreground = parse_color(c);
    }
    if let Some(c) = &colors.danger_foreground {
        theme.danger_foreground = parse_color(c);
    }
    if let Some(c) = &colors.sidebar_foreground {
        theme.sidebar_foreground = parse_color(c);
    }
    if let Some(c) = &colors.button_foreground {
        theme.button_foreground = parse_color(c);
    }
    if let Some(c) = &colors.border {
        theme.border = parse_color(c);
    }
    if let Some(c) = &colors.sidebar_border {
        theme.sidebar_border = parse_color(c);
    }
    if let Some(c) = &colors.input {
        theme.input = parse_color(c);
    }
    if let Some(c) = &colors.button {
        theme.button = parse_color(c);
    }
    if let Some(c) = &colors.button_hover {
        theme.button_hover = parse_color(c);
    }
    if let Some(c) = &colors.button_active {
        theme.button_active = parse_color(c);
    }
    if let Some(c) = &colors.tab {
        theme.tab = parse_color(c);
    }
    if let Some(c) = &colors.tab_active {
        theme.tab_active = parse_color(c);
    }
    if let Some(c) = &colors.tab_foreground {
        theme.tab_foreground = parse_color(c);
    }
    if let Some(c) = &colors.tab_bar {
        theme.tab_bar = parse_color(c);
    }
    if let Some(c) = &colors.caret {
        theme.caret = parse_color(c);
    }
    if let Some(c) = &colors.selection {
        let color = parse_color(c);
        let alpha = if color.a >= 0.99 {
            0.35
        } else {
            color.a.min(0.5)
        };
        theme.selection = color.alpha(alpha);
    }
    if let Some(c) = &colors.ring {
        theme.ring = parse_color(c);
    }
    if let Some(c) = &colors.scrollbar_thumb {
        theme.scrollbar_thumb = parse_color(c);
    }
    if let Some(c) = &colors.red {
        theme.red = parse_color(c);
    }
    if let Some(c) = &colors.red_light {
        theme.red_light = parse_color(c);
    }
    if let Some(c) = &colors.green {
        theme.green = parse_color(c);
    }
    if let Some(c) = &colors.blue {
        theme.blue = parse_color(c);
    }
    if let Some(c) = &colors.yellow {
        theme.yellow = parse_color(c);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSelectItem {
    pub id: String,
    pub label: String,
}

impl SelectItem for ThemeSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}
