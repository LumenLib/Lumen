use gpui::Hsla;
use serde::{Deserialize, Serialize};

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
#[derive(Default)]
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
#[derive(Default)]
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

/// 已解析（最终色值）的表面颜色集，直接供渲染使用。
#[derive(Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// 一套完整主题方案（含 light / dark / 表面色）
pub struct ThemeScheme {
    pub name: String,
    pub light: ThemeColors,
    pub dark: ThemeColors,
    #[serde(default)]
    pub surface_light: SurfaceColors,
    #[serde(default)]
    pub surface_dark: SurfaceColors,
}
