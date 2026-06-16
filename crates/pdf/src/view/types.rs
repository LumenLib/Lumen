use gpui::prelude::*;
use gpui::{App, Context, SharedString, Window, div, rems};
use gpui_component::IconNamed;
use gpui_component::{
    ActiveTheme, IndexPath,
    label::Label,
    list::{ListDelegate, ListItem, ListState},
    v_flex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PageColorMode {
    White,
    Sepia,
    EyeProtect,
}

#[derive(Clone, PartialEq)]
pub enum WorkerState {
    Loading,
    Running,
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftSidebarTab {
    Thumbnails,
    Outline,
    Annotations,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightSidebarTab {
    Translation,
    Notes,
    Chat,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranslationResult {
    pub original: String,
    pub translated: Option<String>,
    pub is_loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfIconName {
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    ClipboardCopy,
    ZoomIn,
    ZoomOut,
    #[cfg(not(target_os = "macos"))]
    Maximize,
    Sidebar,
    Search,
    #[cfg(not(target_os = "macos"))]
    Minimize,
    #[cfg(not(target_os = "macos"))]
    Restore,
    Close,
    Outline,
    Annotations,
    Translate,
    RotateCw,
    Square,
    FitWidth,
    PanelRight,
    FileText,
    Pages,
    Check,
    FastForward,
    Pin,
    MessageSquare,
    Brain,
    Star,
    Zap,
}

impl IconNamed for PdfIconName {
    fn path(self) -> SharedString {
        match self {
            Self::ChevronLeft => "icons/chevron_left.svg".into(),
            Self::ChevronRight => "icons/chevron_right.svg".into(),
            Self::ChevronDown => "icons/chevron_down.svg".into(),
            Self::ClipboardCopy => "icons/copy.svg".into(),
            Self::ZoomIn => "icons/plus.svg".into(),
            Self::ZoomOut => "icons/minus.svg".into(),
            #[cfg(not(target_os = "macos"))]
            Self::Maximize => "icons/maximize.svg".into(),
            Self::Sidebar => "icons/sidebar.svg".into(),
            Self::Search => "icons/search.svg".into(),
            #[cfg(not(target_os = "macos"))]
            Self::Minimize => "icons/minimize.svg".into(),
            #[cfg(not(target_os = "macos"))]
            Self::Restore => "icons/restore.svg".into(),
            Self::Close => "icons/close_small.svg".into(),
            Self::Outline => "icons/list_tree.svg".into(),
            Self::Annotations => "icons/edit.svg".into(),
            Self::Translate => "icons/globe.svg".into(),
            Self::RotateCw => "icons/rotate_cw.svg".into(),
            Self::Square => "icons/square_dashed.svg".into(),
            Self::FitWidth => "icons/rows.svg".into(),
            Self::PanelRight => "icons/panel_right.svg".into(),
            Self::FileText => "icons/note.svg".into(),
            Self::Pages => "icons/layout_grid.svg".into(),
            Self::Check => "icons/check.svg".into(),
            Self::FastForward => "icons/fast_forward.svg".into(),
            Self::Pin => "icons/attachment.svg".into(),
            Self::MessageSquare => "icons/message_square.svg".into(),
            Self::Brain => "icons/brain.svg".into(),
            Self::Star => "icons/star.svg".into(),
            Self::Zap => "icons/zap.svg".into(),
        }
    }
}

pub const SIDEBAR_MIN_RATIO: f32 = 0.1;
pub const SIDEBAR_MAX_RATIO: f32 = 0.4;
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 200.0;

pub const TOOLBAR_HEIGHT_REMS: f32 = 3.0;

// ── 页面布局 ──────────────────────────────────────────
/// display_w 计算公式的基准宽度（rem）。display_w = BASE * zoom * rem_size
pub const PAGE_BASE_WIDTH_REMS: f32 = 45.0;
/// 自动适配宽度时减去的留白/滚动条宽度（逻辑像素）
pub const AUTO_FIT_PADDING_PX: f32 = 48.0;

// ── 缓存容量 ──────────────────────────────────────────
pub const PAGE_CACHE_SIZE: usize = 6;
pub const TEXT_CACHE_SIZE: usize = 10;
pub const LINK_CACHE_SIZE: usize = 10;
pub const THUMBNAIL_CACHE_SIZE: usize = 10;

// ── 渲染缩放量化 ──────────────────────────────────────
/// 渲染缩放等级桶。渲染时取 ≥ 当前显示缩放的最接近桶值。
pub const RENDER_ZOOM_BUCKETS: &[f32] = &[0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0];

/// 将显示缩放量化为渲染缩放：取 ≥ zoom 的最小桶值
pub fn quantize_render_zoom(zoom: f32) -> f32 {
    RENDER_ZOOM_BUCKETS
        .iter()
        .copied()
        .find(|&level| level >= zoom)
        .unwrap_or(*RENDER_ZOOM_BUCKETS.last().unwrap())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub page_index: u16,
    pub start_char: usize,
    pub end_char: usize,
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub results: Vec<SearchMatch>,
    pub active_match_idx: Option<usize>,
}

impl SearchState {
    pub fn total_matches(&self) -> usize {
        self.results.len()
    }

    pub fn active_match(&self) -> Option<&SearchMatch> {
        self.active_match_idx.and_then(|i| self.results.get(i))
    }
}

// ── 搜索结果显示（gpui-component List） ───────────────

/// 预计算的搜索结果显示数据
#[derive(Clone)]
pub struct SearchResultDisplay {
    pub title: String,
    pub context: SharedString,
}

/// 搜索结果的 ListDelegate，用于 gpui-component List 虚拟滚动
pub struct SearchResultsDelegate {
    pub items: Vec<SearchResultDisplay>,
    pub active_match_idx: Option<usize>,
    pub selected_idx: Option<IndexPath>,
}

impl ListDelegate for SearchResultsDelegate {
    type Item = ListItem;

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = &self.items[ix.row];
        if item.context.is_empty() {
            return None;
        }
        let is_active = self.active_match_idx == Some(ix.row);
        let selected = Some(ix) == self.selected_idx;
        let theme = cx.theme();

        Some(
            ListItem::new(ix)
                .selected(selected || is_active)
                .py_2()
                .px_3()
                .border_b_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .gap_y_0p5()
                        .child(Label::new(item.title.clone()).text_sm())
                        .child(
                            div().h(rems(1.25)).overflow_hidden().child(
                                Label::new(item.context.clone())
                                    .text_xs()
                                    .text_color(theme.muted_foreground),
                            ),
                        ),
                ),
        )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_idx = ix;
        cx.notify();
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
    }

    fn cancel(&mut self, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationEngineItem {
    pub value: String,
    pub label: String,
}

impl gpui_component::select::SelectItem for TranslationEngineItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &String {
        &self.value
    }
}
