//! 分析域（analysis）
//!
//! 纯派生 / 外部查询，无 GPUI 依赖、无 DB 连接：
//! - `ccf`：`CCFService` 基于 `ccf_data` 静态表做分级智能匹配。
//! - `ccf_data`：`CCF_RANK_MAP` 静态查表（原属 `database`，与存储无关，随本域迁入）。
//! - `easyscholar`：`fetch_rank` 调用 EasyScholar 接口查询 JCR / 中科院分区。

pub mod ccf;
pub mod ccf_data;
pub mod easyscholar;

pub use ccf::CCFService;
pub use easyscholar::{RankingResult, fetch_rank};
