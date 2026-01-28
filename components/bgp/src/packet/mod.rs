pub mod header;
pub mod open;
pub mod attribute; // 新增
pub mod update;    // 新增


// 方便外部直接 use comp_bgp::packet::BgpHeader;
pub use header::{BgpHeader, BgpMessageType};
pub use open::OpenMessage;
pub use update::UpdateMessage;
