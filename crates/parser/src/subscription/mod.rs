pub mod elsevier;
pub mod ieee;
pub mod rss;

pub use elsevier::ElsevierSubscriptionParser;
pub use ieee::IeeeSubscriptionParser;
pub use rss::RssSubscriptionParser;
