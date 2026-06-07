use models::Literature;
use parser::normalize::author_full_name;
use std::sync::Arc;

/// 搜索匹配结果
#[derive(Debug, Clone)]
pub struct SearchMatch {
    /// 匹配度分数 (可选，用于未来排序)
    pub score: f32,
    /// 匹配的字段类别 (Title, Author, Journal)
    pub field: SearchField,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SearchField {
    Title,
    Author,
    Journal,
    All,
}

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

impl AdvancedSearchQuery {
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

/// 核心搜索器
pub struct SearchEngine;

impl SearchEngine {
    /// 执行基础搜索
    ///
    /// 参数:
    /// - query: 关键词
    /// - items: 待搜索的数据迭代器
    pub fn search<'a>(
        query: &str,
        items: impl IntoIterator<Item = &'a Arc<Literature>>,
    ) -> Vec<&'a Arc<Literature>> {
        let query_trim = query.trim();
        if query_trim.is_empty() {
            return items.into_iter().collect();
        }

        let query_lower = query_trim.to_lowercase();

        items
            .into_iter()
            .filter(|lit| Self::is_match(&query_lower, lit))
            .collect()
    }

    /// 执行高级过滤
    pub fn advanced_search<'a>(
        query: &AdvancedSearchQuery,
        items: impl IntoIterator<Item = &'a Arc<Literature>>,
    ) -> Vec<&'a Arc<Literature>> {
        items
            .into_iter()
            .filter(|lit| {
                // 作者过滤
                if let Some(ref author_q) = query.author {
                    let author_q = author_q.to_lowercase();
                    if !lit
                        .authors
                        .iter()
                        .any(|a| author_full_name(a).to_lowercase().contains(&author_q))
                    {
                        return false;
                    }
                }

                // 年份范围过滤
                if let Some(start) = query.year_start
                    && lit.year.is_none_or(|y| y < start)
                {
                    return false;
                }
                if let Some(end) = query.year_end
                    && lit.year.is_none_or(|y| y > end)
                {
                    return false;
                }

                // 出版物过滤
                if let Some(ref pub_q) = query.publication {
                    let pub_q = pub_q.to_lowercase();
                    let lit_pub = lit
                        .publication
                        .as_ref()
                        .map(|p| p.name.clone())
                        .unwrap_or_default();
                    if lit_pub.is_empty() || !lit_pub.to_lowercase().contains(&pub_q) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// 针对特定字段的搜索
    pub fn search_in_field<'a>(
        query: &str,
        items: impl IntoIterator<Item = &'a Arc<Literature>>,
        field: SearchField,
    ) -> Vec<&'a Arc<Literature>> {
        let query_trim = query.trim();
        if query_trim.is_empty() {
            return items.into_iter().collect();
        }

        let query_lower = query_trim.to_lowercase();

        items
            .into_iter()
            .filter(|lit| match field {
                SearchField::Title => Self::match_title(&query_lower, lit),
                SearchField::Author => Self::match_author(&query_lower, lit),
                SearchField::Journal => Self::match_journal(&query_lower, lit),
                SearchField::All => Self::is_match(&query_lower, lit),
            })
            .collect()
    }

    /// 内部匹配逻辑：判断文献是否满足搜索条件
    fn is_match(query: &str, lit: &Literature) -> bool {
        Self::match_title(query, lit)
            || Self::match_author(query, lit)
            || Self::match_journal(query, lit)
    }

    fn match_title(query: &str, lit: &Literature) -> bool {
        lit.title.to_lowercase().contains(query)
    }

    fn match_author(query: &str, lit: &Literature) -> bool {
        lit.authors.iter().any(|a| {
            a.last_name.to_lowercase().contains(query)
                || a.first_name.to_lowercase().contains(query)
                || author_full_name(a).to_lowercase().contains(query)
        })
    }

    fn match_journal(query: &str, lit: &Literature) -> bool {
        let pub_str = lit
            .publication
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        !pub_str.is_empty() && pub_str.to_lowercase().contains(query)
    }
}
