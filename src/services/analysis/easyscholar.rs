//! `EasyScholar` API 客户端
//!
//! 负责调用 `EasyScholar` 接口查询 JCR 和中科院分区信息。

use anyhow::{Result, anyhow};
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::collections::HashMap;

const API_URL: &str = "https://www.easyscholar.cc/open/getPublicationRank";

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: i32,
    msg: String,
    data: Option<ApiData>,
}

#[derive(Debug, Deserialize)]
struct ApiData {
    // 仅关注官方分区数据 (JCR/中科院)
    #[serde(rename = "officialRank")]
    official_rank: OfficialRank,
}

#[derive(Debug, Deserialize)]
struct OfficialRank {
    all: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct RankingResult {
    pub jcr: Option<String>,
    pub cas: Option<String>,
}

pub async fn fetch_rank(title: &str, secret_key: &str) -> Result<RankingResult> {
    debug!("EasyScholar: 正在查询 '{title}' ...");

    let client = reqwest::Client::new();
    // 手动构建 URL 避免 query 参数序列化问题
    let url = format!("{API_URL}?secretKey={secret_key}&publicationName={title}");

    let resp: ApiResponse = client
        .get(&url)
        .send()
        .await
        .map_err(|e| {
            warn!("EasyScholar: 网络请求失败 '{title}': {e}");
            e
        })?
        .json()
        .await
        .map_err(|e| {
            warn!("EasyScholar: 响应解析失败 '{title}': {e}");
            e
        })?;

    if resp.code != 200 {
        error!("EasyScholar: '{title}' API 返回错误: {}", resp.msg);
        return Err(anyhow!("API Error: {}", resp.msg));
    }

    let data = resp.data.ok_or_else(|| {
        warn!(
            "EasyScholar: '{title}' 返回数据为空 (code={}, msg='{}')",
            resp.code, resp.msg
        );
        anyhow!("No data returned")
    })?;

    // 1. 解析 JCR (sci)
    let jcr_rank = data.official_rank.all.get("sci").cloned();
    if jcr_rank.is_none() {
        warn!("EasyScholar: '{title}' 返回数据中缺少 sci (JCR) 字段");
    }

    // 2. 解析 中科院 (sciBase)
    let cas_rank = data.official_rank.all.get("sciBase").cloned();
    if cas_rank.is_none() {
        warn!("EasyScholar: '{title}' 返回数据中缺少 sciBase (中科院) 字段");
    }

    debug!("EasyScholar 查询结果: JCR={jcr_rank:?}, CAS={cas_rank:?}");

    info!("EasyScholar: '{title}' -> JCR={jcr_rank:?}, CAS={cas_rank:?}");

    Ok(RankingResult {
        jcr: jcr_rank,
        cas: cas_rank,
    })
}
