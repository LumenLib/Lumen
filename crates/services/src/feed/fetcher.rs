use anyhow::{Result, anyhow};
use log::{debug, error, info, warn};
use models::Literature;
use parser::{ArxivParser, BibTeXParser, DblpParser, DoiParser, MetadataParser, OpenAlexParser};

/// 解析管理模块
///
/// 负责协调各种解析后端，为 UI 提供高级异步接口
pub struct FetcherService;

impl Default for FetcherService {
    fn default() -> Self {
        Self::new()
    }
}

impl FetcherService {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 通用解析入口
    async fn fetch_internal(&self, source_id: &str, input: &str) -> Result<Vec<Literature>> {
        debug!(
            "解析管理: 调用 {source_id} 解析器，输入前60字符: '{}'",
            input.chars().take(60).collect::<String>()
        );
        match source_id {
            "DOI" => DoiParser.parse(input).await,
            "ArXiv" => ArxivParser.parse(input).await,
            "DBLP" => DblpParser.parse(input).await,
            "OpenAlex" => OpenAlexParser.parse(input).await,
            "BibTeX" => Ok(BibTeXParser::parse(input)?),
            _ => Err(anyhow!("未知的解析源: {source_id}")),
        }
    }

    pub async fn parse_doi(&self, doi: &str) -> Result<Literature> {
        info!("解析管理: 正在通过 DOI 解析文献: {doi}");
        // 使用通用接口
        let result = self.fetch_internal("DOI", doi).await;

        match result {
            Ok(mut lits) => {
                if let Some(lit) = lits.pop() {
                    info!("解析管理: DOI 解析成功: '{}'", lit.title);
                    Ok(lit)
                } else {
                    debug!("解析管理: DOI 解析返回空结果");
                    Err(anyhow!("DOI 解析未返回结果"))
                }
            }
            Err(e) => {
                error!("解析管理: DOI 解析失败 ({doi}): {e}");
                Err(e)
            }
        }
    }

    /// 从 arXiv ID 解析文献信息
    pub async fn parse_arxiv(&self, arxiv_id: &str) -> Result<Literature> {
        info!("解析管理: 正在通过 arXiv ID 解析文献: {arxiv_id}");
        let result = self.fetch_internal("ArXiv", arxiv_id).await;

        match result {
            Ok(mut lits) => {
                if let Some(lit) = lits.pop() {
                    info!("解析管理: arXiv 解析成功: '{}'", lit.title);
                    Ok(lit)
                } else {
                    Err(anyhow!("arXiv 解析未返回结果"))
                }
            }
            Err(e) => {
                error!("解析管理: arXiv 解析失败 ({arxiv_id}): {e}");
                Err(e)
            }
        }
    }

    /// 解析 BibTeX 文本内容
    pub fn parse_bibtex(&self, content: &str) -> Result<Vec<Literature>> {
        // 暂时保留原来的静态调用方式以兼容现有同步签名
        info!("解析管理: 正在解析 BibTeX 内容 (长度: {})", content.len());
        let result = BibTeXParser::parse(content);
        match &result {
            Ok(lits) => info!("解析管理: BibTeX 解析成功，获取到 {} 篇文献", lits.len()),
            Err(e) => error!("解析管理: BibTeX 解析失败: {e}"),
        }
        result
    }

    /// 从 DBLP 搜索文献
    pub async fn search_dblp(&self, query: &str) -> Result<Vec<Literature>> {
        info!("解析管理: 正在 DBLP 搜索: '{query}'");
        let result = self.fetch_internal("DBLP", query).await;

        match &result {
            Ok(lits) => info!("解析管理: DBLP 搜索完成，获取到 {} 个结果", lits.len()),
            Err(e) => error!("解析管理: DBLP 搜索失败: {e}"),
        }
        result
    }

    /// 从 `OpenAlex` 搜索文献
    pub async fn search_openalex(&self, query: &str, limit: usize) -> Result<Vec<Literature>> {
        info!("解析管理: 正在 OpenAlex 搜索: '{query}' (限制: {limit})");
        // 注意：Trait 接口不支持 limit 参数，如果需要高级控制，
        // 仍需直接调用 OpenAlexParser::search，或者将 limit 编码进 query 字符串（如 query|limit）
        // 为了保持现有功能，这里暂时直接调用静态方法
        let result = OpenAlexParser::search(query, limit).await;

        match &result {
            Ok(lits) => info!("解析管理: OpenAlex 搜索完成，获取到 {} 个结果", lits.len()),
            Err(e) => error!("解析管理: OpenAlex 搜索失败: {e}"),
        }
        result
    }

    /// 通过 `OpenAlex` 解析 DOI
    pub async fn parse_openalex(&self, doi: &str) -> Result<Literature> {
        info!("解析管理: 正在通过 OpenAlex 解析 DOI: {doi}");
        let result = OpenAlexParser::resolve(doi).await;
        match &result {
            Ok(lit) => info!("解析管理: OpenAlex 解析成功: '{}'", lit.title),
            Err(e) => error!("解析管理: OpenAlex 解析失败 ({doi}): {e}"),
        }
        result
    }

    /// 搜索 DBLP 并返回最佳匹配结果
    pub async fn resolve_dblp_best_match(&self, query: &str) -> Result<Literature> {
        info!("解析管理: 正在获取 DBLP 最佳匹配: '{query}'");
        let results = self.search_dblp(query).await?;
        results.into_iter().next().ok_or_else(|| {
            warn!("解析管理: DBLP 搜索 '{query}' 未找到匹配结果");
            anyhow::anyhow!("未找到匹配结果")
        })
    }

    /// 搜索 `OpenAlex` 并返回最佳匹配结果
    pub async fn resolve_openalex_best_match(&self, title: &str) -> Result<Literature> {
        info!("解析管理: 正在获取 OpenAlex 最佳匹配: '{title}'");
        // 保持原有的 limit=5 逻辑
        let results = self.search_openalex(title, 5).await?;
        results.into_iter().next().ok_or_else(|| {
            warn!("解析管理: OpenAlex 搜索 '{title}' 未找到匹配结果");
            anyhow::anyhow!("未找到匹配结果")
        })
    }

    /// 通过 `OpenAlex` 自动解析文献：有 DOI 优先精准匹配，否则回退标题搜索
    pub async fn resolve_openalex_auto(&self, lit: &Literature) -> Result<Option<Literature>> {
        if let Some(doi) = lit.doi.as_ref().filter(|d| !d.trim().is_empty()) {
            self.parse_openalex(doi.trim()).await.map(Some)
        } else if !lit.title.is_empty() {
            self.resolve_openalex_best_match(&lit.title).await.map(Some)
        } else {
            Ok(None)
        }
    }
}
