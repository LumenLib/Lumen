//! 搜索查询与字段（纯领域类型，services 与 UI 共享）

/// 高级搜索查询条件
#[derive(Debug, Clone, Default)]
pub struct AdvancedSearchQuery {
    pub author: Option<String>,
    pub year_start: Option<i32>,
    pub year_end: Option<i32>,
    pub publication: Option<String>,
    pub ccf_level: Option<String>,
    pub jcr_quarter: Option<String>,
}

/// 搜索匹配的字段类别
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchField {
    Title,
    Author,
    Journal,
    All,
}

impl AdvancedSearchQuery {
    /// 是否未设置任何过滤条件
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.author.is_none()
            && self.year_start.is_none()
            && self.year_end.is_none()
            && self.publication.is_none()
            && self.ccf_level.is_none()
            && self.jcr_quarter.is_none()
    }
}
