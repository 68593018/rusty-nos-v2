pub mod header;
pub mod open;

// 方便外部直接 use comp_bgp::packet::BgpHeader;
pub use header::{BgpHeader, BgpMessageType};
pub use open::OpenMessage;

