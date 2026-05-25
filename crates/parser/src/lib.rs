pub mod csl;
pub mod export;
pub mod metadata;
pub mod normalize;
pub mod subscription;
pub mod text;
pub mod time;

pub use metadata::*;
pub use subscription::*;

pub use metadata::ArxivParser;
pub use metadata::BibTeXParser;
pub use metadata::DblpParser;
pub use metadata::DoiParser;
pub use metadata::OpenAlexParser;
pub use subscription::ElsevierSubscriptionParser;
pub use subscription::IeeeSubscriptionParser;
pub use subscription::RssSubscriptionParser;

use anyhow::Result;
use models::Literature;

pub const USER_AGENT: &str = "Lumen/0.1 (mailto:haifeng_dai@seu.edu.cn)";

#[allow(async_fn_in_trait)]
pub trait MetadataParser: Send + Sync {
    fn source_id(&self) -> &str;
    async fn parse(&self, input: &str) -> Result<Vec<Literature>>;
}
