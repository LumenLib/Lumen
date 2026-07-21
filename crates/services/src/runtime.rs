//! 服务层自带的异步运行时。
//!
//! 本 crate 不依赖 `lumen`（UI 组合根），因此需要自己持有一个 Tokio 运行时，
//! 供 `feed` 域的后台刷新 / 自动更新循环使用。
//! 与 `lumen::RUNTIME` 相互独立，进程中允许多个 Tokio 运行时共存。

use std::sync::LazyLock;
use tokio::runtime::Runtime;

pub static RUNTIME: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("Failed to create Tokio runtime for services crate"));
