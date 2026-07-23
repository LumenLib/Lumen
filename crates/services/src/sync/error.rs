//! 同步错误分类与用户友好提示
//!
//! 同步底层（MySQL / WebDAV）抛出的错误往往是驱动原始文案（如
//! `Input/output error: Broken pipe (os error 32)`），直接透传给用户既难懂
//! 也无法指导操作。这里在源头把错误归类为少数几种，并生成包含「用户该怎么做」
//! 的中文提示，供 `SyncStatus::Error` 展示。
//!
//! 设计取舍：保持 `SyncStatus::Error(String)` 不变，仅把存入其中的字符串从
//! 原始错误改为分类后的友好提示；原始错误仍由调用方写进 `error!` 日志，便于排查。

/// 同步失败的大类，决定提示文案与用户引导
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncErrorKind {
    /// 网络不可达 / 连接断开 / 超时（切换网络、服务器掉线等）
    Network,
    /// 账号或密码错误，被服务器拒绝
    Auth,
    /// 数据库结构不一致（表/字段缺失、迁移失败）
    Schema,
    /// 其他未归类的错误
    Unknown,
}

/// 根据底层错误文本判断类别
pub(crate) fn classify_sync_error(e: &anyhow::Error) -> SyncErrorKind {
    let s = e.to_string().to_lowercase();

    if s.contains("access denied")
        || s.contains("denied for user")
        || s.contains("authentication failed")
        || s.contains("login failed")
    {
        SyncErrorKind::Auth
    } else if s.contains("broken pipe")
        || s.contains("os error 32")      // EPIPE
        || s.contains("os error 111")     // ECONNREFUSED
        || s.contains("os error 61")      // macOS ECONNREFUSED
        || s.contains("os error 51")      // ENETUNREACH
        || s.contains("os error 113")     // EHOSTUNREACH
        || s.contains("connection refused")
        || s.contains("connection reset")
        || s.contains("connection aborted")
        || s.contains("network is unreachable")
        || s.contains("host is unreachable")
        || s.contains("no route to host")
        || s.contains("name or service not known")
        || s.contains("dns error")
        || s.contains("timed out")
        || s.contains("timeout")
        || s.contains("error sending request")   // reqwest / WebDAV 网络层
        || s.contains("error trying to connect")
    // reqwest
    {
        SyncErrorKind::Network
    } else if s.contains("table")
        || s.contains("column")
        || s.contains("unknown column")
        || s.contains("migration")
        || s.contains("doesn't exist")
        || s.contains("does not exist")
    {
        SyncErrorKind::Schema
    } else {
        SyncErrorKind::Unknown
    }
}

/// 生成用户可见的友好提示（按已分类的 `kind`）
///
/// `target` 用于指明是哪个远端（如 `同步数据库 192.168.3.3:3306` 或
/// `远程附件存储（WebDAV）`），让用户知道问题出在哪一侧。
pub(crate) fn format_sync_error_with_kind(
    kind: SyncErrorKind,
    e: &anyhow::Error,
    target: &str,
) -> String {
    match kind {
        SyncErrorKind::Network => format!(
            "无法连接{target}：网络不可达或连接已断开（常见于切换网络后）。请检查网络连接，恢复后手动点击“同步”按钮重试；本次自动同步已暂停。"
        ),
        SyncErrorKind::Auth => format!(
            "连接{target}被拒绝：账号或密码错误。请到「设置 → 同步」中检查同步账号配置后重试。"
        ),
        SyncErrorKind::Schema => format!(
            "同步失败：{target}的数据库结构不一致（表或字段缺失）。请查看日志，或尝试在设置中重建远程表结构后重试。"
        ),
        SyncErrorKind::Unknown => format!("同步失败：{e}。如问题持续，请查看日志或稍后重试。"),
    }
}

/// 便捷封装：先分类再生成提示
pub(crate) fn format_sync_error(e: &anyhow::Error, target: &str) -> String {
    format_sync_error_with_kind(classify_sync_error(e), e, target)
}
