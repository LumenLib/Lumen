//! 元数据获取来源（纯领域类型，services 与 UI 共享）

/// 获取元数据的来源
#[derive(Clone)]
pub enum FetchSource {
    ArXiv(String),
    Doi(String),
    Dblp(String),
    OpenAlexDoi(String),
    OpenAlexTitle(String),
}
