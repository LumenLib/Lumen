//! 纯数据类型
//!
//! 仅包含 struct/enum 定义 + serde + Display + Default，不含业务逻辑。

pub mod annotation;
pub mod attachment;
pub mod author;
pub mod chat;
pub mod citation;
pub mod feed;
pub mod folder;
pub mod literature;
pub mod literature_note;
pub mod local_state;
pub mod publication;
pub mod tag;

pub use annotation::{Annotation, AnnotationColor, AnnotationKind, TextRange};
pub use attachment::Attachment;
pub use author::Author;
pub use citation::Citation;
pub use feed::{Feed, FeedItem, FeedType};
pub use folder::{Folder, FolderType};
pub use literature::{Literature, LiteratureType, ReadingStatus};
pub use literature_note::LiteratureNote;
pub use local_state::{AppUiState, PdfState, WindowState};
pub use publication::{Publication, PublicationType};
pub use tag::{DEFAULT_TAG_COLOR, DEFAULT_VERSION, TAG_COLORS, Tag};
