//! 时间戳格式化工具

/// 把 Unix 秒时间戳转成可读字符串（`%Y-%m-%d %H:%M:%S`）。
///
/// 模型层的 `created_at` / `updated_at` 已统一为 `i64`（Unix 秒），
/// UI 显示时调用本函数，避免在各处重复格式化逻辑。
pub fn ts_to_str(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}
