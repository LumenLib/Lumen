use anyhow::Result;
use gpui::{Hsla, SharedString};
use gpui_component::select::SelectItem;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::LazyLock;
use std::{collections::HashMap, fs, path::Path, sync::RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// 基础颜色集 —— 控制整个 app 的基础外观
///
/// 这些字段映射到 gpui-component 的每个 UI 组件（弹窗、输入框、按钮、标签页等）。
/// 在 `one.json` 中同时写入 `light` 和 `dark` 两个版本。
///
/// 色值支持：
///   - 6 位 hex：`#ff0000`
///   - 8 位 hex（带透明度）：`#ff000080`
///   - 命名色：`"red-500"`, `"neutral-200"`
///   - 渐变色
pub struct ThemeColors {
    // ── 文字色 ──
    /// 主文字颜色。大部分正文、标签、列表项的文字
    pub foreground: Option<String>,
    /// 次要文字颜色。提示文字、辅助信息、占位符
    pub muted_foreground: Option<String>,
    /// 弹窗/下拉菜单中的文字颜色。与 popover 背景搭配
    pub popover_foreground: Option<String>,
    /// 链接文字颜色。可点击的链接或超链接样式文字
    pub link: Option<String>,

    // ── 背景色 ──
    /// 应用主背景色。中间文献列表区域、整个窗口的底层背景
    pub background: Option<String>,
    /// 标题栏背景色。窗口最顶部的标题栏底色
    pub title_bar: Option<String>,
    /// 标题栏下边框颜色。标题栏底部的分割线
    pub title_bar_border: Option<String>,
    /// 左侧栏背景色。文件夹/标签/订阅列表的侧边栏底色
    pub sidebar: Option<String>,
    /// 侧栏中选中/高亮区域的颜色。侧栏选中项的背景色
    pub sidebar_accent: Option<String>,
    /// 柔和背景色。设置页内的次级卡片、hover 时的背景
    pub muted: Option<String>,
    /// 强调色背景。列表选中项、选中标签的背景
    pub accent: Option<String>,
    /// 主色调。链接、按钮主色、选中文字、进度条、开关等
    pub primary: Option<String>,
    /// 次要色背景。次要按钮、次要标签的背景
    pub secondary: Option<String>,
    /// 危险/删除色。删除按钮、错误状态的红色
    pub danger: Option<String>,
    /// 成功色。绿色对勾、已完成状态
    pub success: Option<String>,
    /// 警告色。黄色警告标记
    pub warning: Option<String>,
    /// 信息色。蓝色信息提示
    pub info: Option<String>,
    /// 弹窗/下拉菜单背景色。弹出菜单、下拉选择框的底色
    pub popover: Option<String>,
    /// 状态栏背景色。底部状态栏底色
    pub status_bar: Option<String>,
    /// 状态栏边框色。状态栏顶部分割线
    pub status_bar_border: Option<String>,
    /// 骨架屏加载态背景。内容未加载完时的占位色块
    pub skeleton: Option<String>,
    /// 进度条颜色。加载进度条、扫描进度条的填充色
    pub progress_bar: Option<String>,
    /// 遮罩层颜色。弹窗背后半透明遮罩
    pub overlay: Option<String>,

    // ── 前景/强调文字 ──
    /// 强调色上的文字颜色。与 accent 背景搭配的文字
    pub accent_foreground: Option<String>,
    /// 主色上的文字颜色。与 primary 背景搭配的文字
    pub primary_foreground: Option<String>,
    /// 危险色上的文字颜色。与 danger 背景搭配的文字
    pub danger_foreground: Option<String>,
    /// 侧栏中的文字颜色。侧栏列表项的文字
    pub sidebar_foreground: Option<String>,
    /// 按钮上的文字颜色。普通按钮中的文字
    pub button_foreground: Option<String>,

    // ── 边框 ──
    /// 通用边框色。卡片、弹窗、输入框的边框
    pub border: Option<String>,
    /// 侧栏边框色。侧栏与中间区域之间的分割线
    pub sidebar_border: Option<String>,
    /// 输入框边框色。输入框、下拉框的边框
    pub input: Option<String>,

    // ── 控件色 ──
    /// 按钮背景色（默认状态）
    pub button: Option<String>,
    /// 按钮悬浮背景色（鼠标悬停时）
    pub button_hover: Option<String>,
    /// 按钮按下背景色（鼠标按下时）
    pub button_active: Option<String>,
    /// 标签页背景色（非活动标签）
    pub tab: Option<String>,
    /// 标签页背景色（活动标签）
    pub tab_active: Option<String>,
    /// 标签页文字颜色
    pub tab_foreground: Option<String>,
    /// 标签栏背景色（标签页所在条的背景）
    pub tab_bar: Option<String>,
    /// 输入光标颜色。输入框中闪动的竖线
    pub caret: Option<String>,
    /// 文字选中高亮色。鼠标拖动选择文字时的背景
    pub selection: Option<String>,
    /// 焦点环颜色。输入框聚焦时的外发光环
    pub ring: Option<String>,
    /// 滚动条滑块颜色。可滚动区域的滚动条滑块
    pub scrollbar_thumb: Option<String>,

    // ── 功能色 ──
    /// 红色（语义色）。用于错误、删除、危险图标
    pub red: Option<String>,
    /// 浅红色。比 red 更浅的红色，用于次要错误提示
    pub red_light: Option<String>,
    /// 绿色（语义色）。用于成功、完成的图标和文字
    pub green: Option<String>,
    /// 蓝色（语义色）。用于信息、链接的图标
    pub blue: Option<String>,
    /// 黄色（语义色）。用于警告、注意的图标
    pub yellow: Option<String>,
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self {
            foreground: None,
            muted_foreground: None,
            popover_foreground: None,
            link: None,
            background: None,
            title_bar: None,
            title_bar_border: None,
            sidebar: None,
            sidebar_accent: None,
            muted: None,
            accent: None,
            primary: None,
            secondary: None,
            danger: None,
            success: None,
            warning: None,
            info: None,
            popover: None,
            status_bar: None,
            status_bar_border: None,
            skeleton: None,
            progress_bar: None,
            overlay: None,
            accent_foreground: None,
            primary_foreground: None,
            danger_foreground: None,
            sidebar_foreground: None,
            button_foreground: None,
            border: None,
            sidebar_border: None,
            input: None,
            button: None,
            button_hover: None,
            button_active: None,
            tab: None,
            tab_active: None,
            tab_foreground: None,
            tab_bar: None,
            caret: None,
            selection: None,
            ring: None,
            scrollbar_thumb: None,
            red: None,
            red_light: None,
            green: None,
            blue: None,
            yellow: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
/// 表面颜色集 —— 控制各种交互状态的衍生色
///
/// 这些字段是 Lumen 自定义的，不来自 gpui-component。
/// 它们原本是通过 `theme.xxx.opacity(0.N)` 计算得到的视觉层次，
/// 现在改为直接配置最终色值，便于你精确控制每个交互状态。
///
/// 在 `one.json` 中写入 `surface_light` / `surface_dark` 两个对象。
/// 色值使用 8 位 hex（RRGGBBAA），末尾两位是透明度：
///   `19` = 10%,  `4d` = 30%,  `80` = 50%,  `cc` = 80%,  `e6` = 90%
///
/// 字段按功能分组而非透明度分组。
pub struct SurfaceColors {
    // ── 选中 / 活动状态 ──
    /// 【选中计数字色】选中项右侧数量的文字颜色
    pub selected_text: Option<String>,
    /// 【极弱选中背景】拖放文件夹区域、文献对比表中选中差异列的极微弱高亮
    pub selected_faint: Option<String>,
    /// 【拖放目标悬停】拖拽文献到文件夹时，目标文件夹的背景高亮
    pub selected_hover: Option<String>,

    // ── 悬停状态 ──
    /// 【标签选择器选中】弹窗中标签选择器的当前聚焦项背景
    pub hover_bg: Option<String>,
    /// 【列表行悬停】中间文献列表每行悬浮时的背景
    pub hover_highlight: Option<String>,
    /// 【侧栏悬停】侧栏「标签」header 悬浮时背景
    pub hover_folder: Option<String>,

    // ── 按钮 ──
    /// 【危险按钮悬停】所有弹窗的关闭/删除按钮悬浮时的红色背景
    pub danger_hover: Option<String>,
    /// 【标签关闭悬停】主窗口标签页上关闭按钮悬浮时的背景
    pub danger_ghost: Option<String>,
    /// 【窗口控制按钮悬停】设置窗口最小化/最大化按钮悬浮背景
    pub window_button_hover: Option<String>,

    // ── 区域层次 ──
    /// 【AI 卡片背景】设置页 AI 供应商卡片底色
    pub card_bg: Option<String>,
    /// 【可展开区域背景】同步设置、代理设置等折叠区域底色
    pub section_bg: Option<String>,
    /// 【提示框背景】“无需 API Key” 等提示信息框底色
    pub info_bg: Option<String>,

    // ── 边框 / 分隔线 ──
    /// 【弱分隔线】标签容器上边框、设置窗口参数区域分割线等弱线条
    pub border_faint: Option<String>,

    // ── 功能杂项 ──
    /// 【模式选择器激活态】主题模式（浅色/深色/跟随系统）选中项的底色
    pub chip_bg: Option<String>,
    /// 【侧栏非活动标签】设置页左侧「通用」「同步」「AI」等非活动标签的文字色
    pub sidebar_tab_inactive: Option<String>,
    /// 【底部拖放遮罩】文献详情页底部文件拖放提示区域的半透明遮罩
    pub drop_overlay: Option<String>,
    /// 【禁用标签文字】输入框禁用时标签文字的颜色
    pub label_disabled: Option<String>,
}

impl Default for SurfaceColors {
    fn default() -> Self {
        Self {
            selected_text: None,
            selected_faint: None,
            selected_hover: None,
            hover_bg: None,
            hover_highlight: None,
            hover_folder: None,
            danger_hover: None,
            danger_ghost: None,
            window_button_hover: None,
            card_bg: None,
            section_bg: None,
            info_bg: None,
            border_faint: None,
            chip_bg: None,
            sidebar_tab_inactive: None,
            drop_overlay: None,
            label_disabled: None,
        }
    }
}

pub struct ResolvedSurface {
    pub selected_text: Hsla,
    pub selected_faint: Hsla,
    pub selected_hover: Hsla,
    pub hover_bg: Hsla,
    pub hover_highlight: Hsla,
    pub hover_folder: Hsla,
    pub danger_hover: Hsla,
    pub danger_ghost: Hsla,
    pub window_button_hover: Hsla,
    pub card_bg: Hsla,
    pub section_bg: Hsla,
    pub info_bg: Hsla,
    pub border_faint: Hsla,
    pub chip_bg: Hsla,
    pub sidebar_tab_inactive: Hsla,
    pub drop_overlay: Hsla,
    pub label_disabled: Hsla,
}

impl Default for ResolvedSurface {
    fn default() -> Self {
        Self {
            selected_text: gpui::transparent_black(),
            selected_faint: gpui::transparent_black(),
            selected_hover: gpui::transparent_black(),
            hover_bg: gpui::transparent_black(),
            hover_highlight: gpui::transparent_black(),
            hover_folder: gpui::transparent_black(),
            danger_hover: gpui::transparent_black(),
            danger_ghost: gpui::transparent_black(),
            window_button_hover: gpui::transparent_black(),
            card_bg: gpui::transparent_black(),
            section_bg: gpui::transparent_black(),
            info_bg: gpui::transparent_black(),
            border_faint: gpui::transparent_black(),
            chip_bg: gpui::transparent_black(),
            sidebar_tab_inactive: gpui::transparent_black(),
            drop_overlay: gpui::transparent_black(),
            label_disabled: gpui::transparent_black(),
        }
    }
}

pub static SURFACE: LazyLock<RwLock<ResolvedSurface>> =
    LazyLock::new(|| RwLock::new(ResolvedSurface::default()));

pub fn surface() -> impl Deref<Target = ResolvedSurface> {
    SURFACE.read().expect("theme surface poisoned")
}

pub fn parse_color(hex: &str) -> Hsla {
    gpui_component::try_parse_color(hex).unwrap_or(gpui::black())
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

impl SurfaceColors {
    pub fn resolve(&self, is_dark: bool) -> ResolvedSurface {
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
            selected_text: resolve_field(&self.selected_text, def.selected_text),
            selected_faint: resolve_field(&self.selected_faint, def.selected_faint),
            selected_hover: resolve_field(&self.selected_hover, def.selected_hover),
            hover_bg: resolve_field(&self.hover_bg, def.hover_bg),
            hover_highlight: resolve_field(&self.hover_highlight, def.hover_highlight),
            hover_folder: resolve_field(&self.hover_folder, def.hover_folder),
            danger_hover: resolve_field(&self.danger_hover, def.danger_hover),
            danger_ghost: resolve_field(&self.danger_ghost, def.danger_ghost),
            window_button_hover: resolve_field(&self.window_button_hover, def.window_button_hover),
            card_bg: resolve_field(&self.card_bg, def.card_bg),
            section_bg: resolve_field(&self.section_bg, def.section_bg),
            info_bg: resolve_field(&self.info_bg, def.info_bg),
            border_faint: resolve_field(&self.border_faint, def.border_faint),
            chip_bg: resolve_field(&self.chip_bg, def.chip_bg),
            sidebar_tab_inactive: resolve_field(
                &self.sidebar_tab_inactive,
                def.sidebar_tab_inactive,
            ),
            drop_overlay: resolve_field(&self.drop_overlay, def.drop_overlay),
            label_disabled: resolve_field(&self.label_disabled, def.label_disabled),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeScheme {
    pub name: String,
    pub light: ThemeColors,
    pub dark: ThemeColors,
    #[serde(default)]
    pub surface_light: SurfaceColors,
    #[serde(default)]
    pub surface_dark: SurfaceColors,
}

impl ThemeColors {
    pub fn apply_to_palette(&self, theme: &mut gpui_component::Theme) {
        if let Some(c) = &self.foreground {
            theme.foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.muted_foreground {
            theme.muted_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.popover_foreground {
            theme.popover_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.link {
            theme.link = self.parse_hex(c);
        }
        if let Some(c) = &self.background {
            theme.background = self.parse_hex(c);
        }
        if let Some(c) = &self.title_bar {
            theme.title_bar = self.parse_hex(c);
        }
        if let Some(c) = &self.title_bar_border {
            theme.title_bar_border = self.parse_hex(c);
        }
        if let Some(c) = &self.sidebar {
            theme.sidebar = self.parse_hex(c);
        }
        if let Some(c) = &self.sidebar_accent {
            theme.sidebar_accent = self.parse_hex(c);
        }
        if let Some(c) = &self.muted {
            theme.muted = self.parse_hex(c);
        }
        if let Some(c) = &self.accent {
            theme.accent = self.parse_hex(c);
        }
        if let Some(c) = &self.primary {
            theme.primary = self.parse_hex(c);
        }
        if let Some(c) = &self.secondary {
            theme.secondary = self.parse_hex(c);
        }
        if let Some(c) = &self.danger {
            theme.danger = self.parse_hex(c);
        }
        if let Some(c) = &self.success {
            theme.success = self.parse_hex(c);
        }
        if let Some(c) = &self.warning {
            theme.warning = self.parse_hex(c);
        }
        if let Some(c) = &self.info {
            theme.info = self.parse_hex(c);
        }
        if let Some(c) = &self.popover {
            theme.popover = self.parse_hex(c);
        }
        if let Some(c) = &self.status_bar {
            theme.status_bar = self.parse_hex(c);
        }
        if let Some(c) = &self.status_bar_border {
            theme.status_bar_border = self.parse_hex(c);
        }
        if let Some(c) = &self.skeleton {
            theme.skeleton = self.parse_hex(c);
        }
        if let Some(c) = &self.progress_bar {
            theme.progress_bar = self.parse_hex(c);
        }
        if let Some(c) = &self.overlay {
            theme.overlay = self.parse_hex(c);
        }
        if let Some(c) = &self.accent_foreground {
            theme.accent_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.primary_foreground {
            theme.primary_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.danger_foreground {
            theme.danger_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.sidebar_foreground {
            theme.sidebar_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.button_foreground {
            theme.button_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.border {
            theme.border = self.parse_hex(c);
        }
        if let Some(c) = &self.sidebar_border {
            theme.sidebar_border = self.parse_hex(c);
        }
        if let Some(c) = &self.input {
            theme.input = self.parse_hex(c);
        }
        if let Some(c) = &self.button {
            theme.button = self.parse_hex(c);
        }
        if let Some(c) = &self.button_hover {
            theme.button_hover = self.parse_hex(c);
        }
        if let Some(c) = &self.button_active {
            theme.button_active = self.parse_hex(c);
        }
        if let Some(c) = &self.tab {
            theme.tab = self.parse_hex(c);
        }
        if let Some(c) = &self.tab_active {
            theme.tab_active = self.parse_hex(c);
        }
        if let Some(c) = &self.tab_foreground {
            theme.tab_foreground = self.parse_hex(c);
        }
        if let Some(c) = &self.tab_bar {
            theme.tab_bar = self.parse_hex(c);
        }
        if let Some(c) = &self.caret {
            theme.caret = self.parse_hex(c);
        }
        if let Some(c) = &self.selection {
            theme.selection = self.parse_hex(c);
        }
        if let Some(c) = &self.ring {
            theme.ring = self.parse_hex(c);
        }
        if let Some(c) = &self.scrollbar_thumb {
            theme.scrollbar_thumb = self.parse_hex(c);
        }
        if let Some(c) = &self.red {
            theme.red = self.parse_hex(c);
        }
        if let Some(c) = &self.red_light {
            theme.red_light = self.parse_hex(c);
        }
        if let Some(c) = &self.green {
            theme.green = self.parse_hex(c);
        }
        if let Some(c) = &self.blue {
            theme.blue = self.parse_hex(c);
        }
        if let Some(c) = &self.yellow {
            theme.yellow = self.parse_hex(c);
        }
    }

    fn parse_hex(&self, hex: &str) -> gpui::Hsla {
        parse_color(hex)
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

impl Default for ThemeLoader {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ThemeLoader {
    themes: HashMap<String, ThemeScheme>,
}

impl ThemeLoader {
    #[must_use]
    pub fn new() -> Self {
        Self {
            themes: HashMap::new(),
        }
    }

    pub fn load_all(&mut self, themes_dir: &Path) -> Result<()> {
        if !themes_dir.exists() {
            let _ = fs::create_dir_all(themes_dir);
            return Ok(());
        }

        for entry in fs::read_dir(themes_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                if let Ok(scheme) = serde_json::from_str::<ThemeScheme>(&content) {
                    log::info!("加载自定义主题: {}", scheme.name);
                    self.themes.insert(scheme.name.clone(), scheme);
                }
            }
        }
        Ok(())
    }

    pub fn load_from_string(&mut self, content: &str) -> Result<()> {
        if let Ok(scheme) = serde_json::from_str::<ThemeScheme>(content) {
            log::info!("加载内置主题: {}", scheme.name);
            self.themes.insert(scheme.name.clone(), scheme);
        }
        Ok(())
    }

    #[must_use]
    pub fn get_theme(&self, name: &str) -> Option<&ThemeScheme> {
        self.themes.get(name)
    }

    #[must_use]
    pub fn available_themes(&self) -> Vec<String> {
        let mut names: Vec<String> = self.themes.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn reload_theme_from_file(&mut self, path: &Path) -> Result<()> {
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(path)?;
            if let Ok(scheme) = serde_json::from_str::<ThemeScheme>(&content) {
                log::info!("热加载主题: {}", scheme.name);
                self.themes.insert(scheme.name.clone(), scheme);
            }
        }
        Ok(())
    }
}

pub static LOADER: LazyLock<RwLock<ThemeLoader>> =
    LazyLock::new(|| RwLock::new(ThemeLoader::new()));
