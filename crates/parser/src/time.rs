use chrono::{DateTime, NaiveDateTime};
use log::debug;

fn parse_time_string(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }

    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Some(dt.naive_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(dt);
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_utc());
    }

    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%d %b %Y %H:%M:%S") {
        return Some(dt);
    }
    if let Ok(d) = chrono::NaiveDate::parse_from_str(s, "%d %b %Y") {
        return d.and_hms_opt(0, 0, 0);
    }

    None
}

pub fn normalize_time_string(s: &str) -> String {
    debug!("时间规格化: 输入=\"{}\"", s);
    if let Some(dt) = parse_time_string(s) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        s.to_string()
    }
}
