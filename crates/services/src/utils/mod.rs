//! 工具域（utils）
//!
//! 纯工具函数（如文件名清洗 `filename`），无 DB / 无 GPUI 依赖。

pub mod filename;

pub use filename::{
    FilenameOptions, filename_options_from_literature, filename_options_from_path,
    generate_literature_filename,
};
