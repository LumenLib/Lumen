//! 时间处理工具
//!
//! 提供时间解析、格式化和规格化功能

use chrono::{DateTime, NaiveDateTime};
use log::debug;

/// 尝试解析各种格式的时间字符串
///
/// 支持的格式：
/// - RFC 2822 (例如: "Wed, 17 Sep 2025 09:17:43 -0400")
/// - `MySQL` 格式 (例如: "2025-09-17 09:17:43")
/// - ISO 8601 (例如: "2025-09-17T09:17:43Z")
#[must_use]
pub fn parse_time_string(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    // 1. 尝试解析 RFC 2822
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        debug!(
            "时间解析: RFC 2822 成功, 输入=\"{}\", 结果={}",
            s,
            dt.naive_utc()
        );
        return Some(dt.naive_utc());
    }

    // 2. 尝试解析 MySQL 格式
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        debug!("时间解析: MySQL 格式成功, 输入=\"{}\", 结果={}", s, dt);
        return Some(dt);
    }

    // 3. 尝试解析 ISO 8601
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        debug!(
            "时间解析: ISO 8601 成功, 输入=\"{}\", 结果={}",
            s,
            dt.naive_utc()
        );
        return Some(dt.naive_utc());
    }

    // 4. 处理 IEEE RSS 中常见的一种非标准格式
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%d %b %Y %H:%M:%S") {
        debug!(
            "时间解析: RSS 日期时间格式成功, 输入=\"{}\", 结果={}",
            s, dt
        );
        return Some(dt);
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%d %b %Y") {
        debug!("时间解析: RSS 日期格式成功, 输入=\"{}\", 结果={}", s, d);
        return d.and_hms_opt(0, 0, 0);
    }

    debug!("时间解析: 所有格式均失败, 输入=\"{}\"", s);
    None
}

/// 将时间字符串规范化为 `MySQL` 兼容的字符串格式 ("YYYY-MM-DD HH:MM:SS")
#[must_use]
pub fn normalize_time_string(s: &str) -> String {
    if let Some(dt) = parse_time_string(s) {
        let result = dt.format("%Y-%m-%d %H:%M:%S").to_string();
        debug!("时间规格化: 输入=\"{}\", 结果=\"{}\"", s, result);
        result
    } else {
        debug!("时间规格化: 无法解析, 返回原值=\"{}\"", s);
        s.to_string()
    }
}
