use serde::{Deserialize, Serialize};
use super::primitives::MacAddress; // 假设您已有 primitives

// 接口物理状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperState {
    Up,
    Down,
    Unknown,
}

// 接口核心信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceEntry {
    pub name: String,          // e.g. "eth0"
    pub ifindex: u32,          // e.g. 2
    pub state: OperState,      // e.g. Up
    pub mtu: u32,              // e.g. 1500
    pub mac: MacAddress,       // e.g. 00:11:22...
}

// 广播事件：告诉 BGP/OSPF 发生了什么
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InterfaceEvent {
    LinkUp(InterfaceEntry),       // 接口 Up (携带全量信息)
    LinkDown(String),             // 接口 Down (只携带名字)
    MtuChanged(String, u32),      // MTU 变更
    AddressAdded(String, String), // IP 增加
}