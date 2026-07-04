/// 获取元数据的来源
#[derive(Clone)]
pub enum FetchSource {
    ArXiv(String),
    Doi(String),
    Dblp(String),
    OpenAlexDoi(String),
    OpenAlexTitle(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchSource {
    ArXiv,
    Doi,
    Dblp,
    OpenAlex,
}

/// 主窗口标签页
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TabId {
    Main,
    Pdf(String), // document_id
}

/// 视图事件
pub enum ViewEvent {
    /// 关闭所有菜单
    CloseMenu,
}
