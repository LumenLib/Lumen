use gpui::SharedString;
use gpui_component::IconNamed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    File,
    FileSolid,
    Attachment,
    Trash,
    Settings,
    Add,
    Clear,
    Edit,
    Folder,
    Globe,
    Cloud,
    Puzzle,
    Info,
    FolderSelect,
    ArrowUpDown,
    Minimize,
    Maximize,
    Restore,
    Close,
    Copy,
    Check,
    FolderOpen,
    BookOpen,
    Home,
    Plus,
    Inbox,
    ChevronDown,
    ChevronRight,
    LoaderCircle,
    CircleX,
    TriangleAlert,
    Undo,
    Bell,
    Star,
}

impl IconNamed for IconName {
    fn path(self) -> SharedString {
        match self {
            // ── Lumen 独有（无依赖等价） ──
            Self::FileSolid => "icons/file_solid.svg".into(),
            Self::Attachment => "icons/attachment.svg".into(),
            Self::Trash => "icons/trash.svg".into(),
            Self::Add => "icons/add.svg".into(),
            Self::Edit => "icons/edit.svg".into(),
            Self::Cloud => "icons/cloud.svg".into(),
            Self::Puzzle => "icons/puzzle.svg".into(),
            Self::Info => "icons/info.svg".into(),
            Self::FolderSelect => "icons/folder_select.svg".into(),
            Self::ArrowUpDown => "icons/arrow_up_down.svg".into(),
            Self::Restore => "icons/restore.svg".into(),
            Self::Home => "icons/home.svg".into(),

            // ── 使用 gpui-component 版本 ──
            Self::File => "icons/file.svg".into(),
            Self::FolderOpen => "icons/folder-open.svg".into(),
            Self::BookOpen => "icons/book-open.svg".into(),
            Self::Minimize => "icons/minimize.svg".into(),
            Self::Maximize => "icons/maximize.svg".into(),

            // ── 路径名一致，直接走依赖 ──
            Self::Settings => "icons/settings.svg".into(),
            Self::Globe => "icons/globe.svg".into(),
            Self::Copy => "icons/copy.svg".into(),
            Self::Check => "icons/check.svg".into(),
            Self::Plus => "icons/plus.svg".into(),
            Self::Inbox => "icons/inbox.svg".into(),
            Self::Undo => "icons/undo.svg".into(),
            Self::Bell => "icons/bell.svg".into(),
            Self::Star => "icons/star.svg".into(),

            // ── 路径名不同但内容一致，指向依赖的命名 ──
            Self::ChevronDown => "icons/chevron-down.svg".into(),
            Self::ChevronRight => "icons/chevron-right.svg".into(),
            Self::CircleX => "icons/circle-x.svg".into(),
            Self::LoaderCircle => "icons/loader-circle.svg".into(),
            Self::TriangleAlert => "icons/triangle-alert.svg".into(),
            Self::Folder => "icons/folder.svg".into(),
            Self::Close => "icons/close.svg".into(),
            Self::Clear => "icons/circle-x.svg".into(),
        }
    }
}
