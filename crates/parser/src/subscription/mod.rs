pub mod elsevier;
pub mod ieee;
pub mod nature;

pub use elsevier::ElsevierSubscriptionParser;
pub use ieee::IeeeSubscriptionParser;
pub use nature::NatureSubscriptionParser;
