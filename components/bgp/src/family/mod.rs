use serde::{Deserialize, Serialize};

/// AFI: Address Family Identifier (RFC 1700)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum Afi {
    Ipv4 = 1,
    Ipv6 = 2,
    L2Vpn = 25,
    // 其他暂不实现
}

/// SAFI: Subsequent Address Family Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Safi {
    Unicast = 1,
    Multicast = 2,
    MplsVpn = 128,
    Evpn = 70,
}

/// 组合结构：AFI + SAFI 唯一确定一个地址族
/// 例如: IPv4 Unicast (1, 1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AfiSafi {
    pub afi: Afi,
    pub safi: Safi,
}

impl AfiSafi {
    pub fn new(afi: Afi, safi: Safi) -> Self {
        Self { afi, safi }
    }
    
    // 辅助方法：判断是否为 IPv4 Unicast
    pub fn is_ipv4_unicast(&self) -> bool {
        matches!(self, AfiSafi { afi: Afi::Ipv4, safi: Safi::Unicast })
    }
}

// 默认支持 IPv4 Unicast
impl Default for AfiSafi {
    fn default() -> Self {
        Self { afi: Afi::Ipv4, safi: Safi::Unicast }
    }
}
