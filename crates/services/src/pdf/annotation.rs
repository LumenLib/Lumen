use models::{Annotation, AnnotationColor};
use std::collections::{HashMap, HashSet};

/// 不依赖 GPUI 的二维点（引擎层几何类型，供二进制侧视图使用）
#[derive(Clone, Copy, Debug)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

/// 逻辑像素长度（不依赖 GPUI）
#[derive(Clone, Copy, Debug)]
pub struct Pixels(pub f32);

impl From<Pixels> for f32 {
    fn from(p: Pixels) -> f32 {
        p.0
    }
}

/// 当前激活的注释工具
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationTool {
    Select,
    Highlight(AnnotationColor),
    Underline(AnnotationColor),
    Rectangle(AnnotationColor),
    Pin,
}

/// 浮动工具栏状态（选中文字后出现）
pub struct AnnotationToolbarState {
    pub start_page: u16,
    pub start_char: usize,
    pub end_page: u16,
    pub end_char: usize,
}

/// 浮动工具栏当前的注释类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarAnnotationKind {
    Highlight,
    Underline,
}

/// 运行时注释状态
pub struct AnnotationState {
    pub active_tool: AnnotationTool,
    pub annotations: HashMap<u16, Vec<Annotation>>,
    pub dirty_ids: HashSet<String>,
    pub selected_id: Option<String>,
    pub toolbar: Option<AnnotationToolbarState>,
    pub toolbar_kind: ToolbarAnnotationKind,
    pub note_editor: Option<NoteEditorState>,
    pub last_highlight_color: AnnotationColor,
}

impl Default for AnnotationState {
    fn default() -> Self {
        Self {
            active_tool: AnnotationTool::Select,
            annotations: HashMap::new(),
            dirty_ids: HashSet::new(),
            selected_id: None,
            toolbar: None,
            toolbar_kind: ToolbarAnnotationKind::Highlight,
            note_editor: None,
            last_highlight_color: AnnotationColor::Yellow,
        }
    }
}

pub struct NoteEditorState {
    pub annotation_id: String,
    pub position: Point<Pixels>,
}
