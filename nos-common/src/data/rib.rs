use ipnet::IpNet;
use std::net::IpAddr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouteProtocol {
    Static,
    BGP,
    OSPF,
}

// 必须实现 Default，否则无法使用 ..Default::default()
impl Default for RouteProtocol {
    fn default() -> Self {
        Self::Static
    }
}

#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub protocol: RouteProtocol,
    pub prefix: IpNet,
    pub nexthop: IpAddr,
    pub metric: u32,
    pub tag: Option<u32>, // 预留字段
}

impl Default for RouteEntry {
    fn default() -> Self {
        Self {
            protocol: RouteProtocol::default(),
            prefix: "0.0.0.0/0".parse().unwrap(),
            nexthop: "0.0.0.0".parse().unwrap(),
            metric: 0,
            tag: None,
        }
    }
}