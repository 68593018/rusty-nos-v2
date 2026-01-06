use std::net::{IpAddr, SocketAddr};
use tokio::net::TcpStream;
use tokio::time::Instant;
use crate::family::AfiSafi;

/// BGP 状态机状态 (RFC 4271)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Connect,
    Active,
    OpenSent,
    OpenConfirm,
    Established,
}

/// 邻居配置 (静态配置)
#[derive(Debug, Clone)]
pub struct PeerConfig {
    pub neighbor_ip: IpAddr,
    pub remote_as: u32,
    pub local_as: u32,
    pub router_id: std::net::Ipv4Addr, // BGP Router ID 必须是 IPv4 格式
    pub hold_time: u16,               // 默认 180s
    pub keepalive_time: u16,          // 默认 60s
    pub enabled_families: Vec<AfiSafi>, // 启用的地址族列表
}

/// 邻居运行时状态 (动态变化)
pub struct Peer {
    // --- 静态部分 ---
    pub config: PeerConfig,

    // --- 动态部分 ---
    pub state: SessionState,
    
    // TCP 连接句柄 (Option 因为在 Idle/Connect 状态下可能没有连接)
    pub stream: Option<TcpStream>,
    
    // 统计信息
    pub uptime: Option<Instant>,
    pub rx_count: u64,
    pub tx_count: u64,
    
    // 协商后的能力 (Capabilities)
    pub negotiated_hold_time: u16,
}

impl Peer {
    pub fn new(config: PeerConfig) -> Self {
        Self {
            config,
            state: SessionState::Idle,
            stream: None,
            uptime: None,
            rx_count: 0,
            tx_count: 0,
            negotiated_hold_time: 0,
        }
    }

    // 状态转换辅助函数
    pub fn change_state(&mut self, new_state: SessionState) {
        println!("BGP Peer {} 状态迁移: {:?} -> {:?}", 
            self.config.neighbor_ip, self.state, new_state);
        self.state = new_state;
        
        if new_state == SessionState::Established {
            self.uptime = Some(Instant::now());
        } else if new_state == SessionState::Idle {
            self.stream = None; // 断开连接
            self.uptime = None;
        }
    }
}
