use bytes::{Buf, BufMut, BytesMut};
use std::io::{self, Cursor};
use std::net::Ipv4Addr;

/// OPEN 消息 (RFC 4271 Sec 4.2)
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+
/// |    Version    |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |     My Autonomous System      |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |           Hold Time           |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                         BGP Identifier                        |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// | Opt Parm Len  |
/// +-+-+-+-+-+-+-+-+
#[derive(Debug, Clone)]
pub struct OpenMessage {
    pub version: u8,    // 必须是 4
    pub my_as: u16,     // 本地 AS 号
    pub hold_time: u16,
    pub bgp_id: Ipv4Addr,
    // Capabilities 暂时略过，后面 Phase 1.x 再加
}

impl OpenMessage {
    pub fn new(my_as: u16, hold_time: u16, bgp_id: Ipv4Addr) -> Self {
        Self {
            version: 4,
            my_as,
            hold_time,
            bgp_id,
        }
    }

    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u8(self.version);
        buf.put_u16(self.my_as);
        buf.put_u16(self.hold_time);
        buf.put_slice(&self.bgp_id.octets());
        buf.put_u8(0); // Opt Param Len = 0 (暂时不带能力协商)
    }
    
    // decode 暂时不写，我们先跑起来发送逻辑
}
