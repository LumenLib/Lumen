use gpui::{Pixels, Point};
use models::{Annotation, AnnotationColor};
use std::collections::{HashMap, HashSet};

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
    pub context_menu: Option<ContextMenuState>,
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
            context_menu: None,
            note_editor: None,
            last_highlight_color: AnnotationColor::Yellow,
        }
    }
}

pub struct ContextMenuState {
    pub annotation_id: String,
    pub position: Point<Pixels>,
    pub from_sidebar: bool,
}

pub struct NoteEditorState {
    pub annotation_id: String,
    pub position: Point<Pixels>,
}
