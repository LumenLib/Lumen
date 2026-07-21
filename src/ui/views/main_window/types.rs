/// 获取元数据的来源（定义已迁至 `models`，此处重导出以保持旧路径可用）
pub use models::FetchSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchSource {
    ArXiv,
    Doi,
    Dblp,
    OpenAlex,
}

/// 视图事件
pub enum ViewEvent {
    /// 关闭所有菜单
    CloseMenu,
}
