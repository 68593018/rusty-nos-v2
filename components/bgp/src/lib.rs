use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use ipnet::IpNet;

// 引入公共定义
use nos_common::services::{RibService, InterfaceService};
use nos_common::data::rib::{RouteEntry, RouteProtocol};
use nos_common::data::interface::{InterfaceEvent, OperState};

// === 新增模块定义 ===
pub mod family;
pub mod packet; 
// pub mod fsm;    // 暂时注释
mod peer;

use peer::{Peer, PeerConfig};
use family::AfiSafi;

use tokio::net::TcpListener;
use crate::peer::SessionState;

// =========================================================
// 1. 新的数据结构 (RFC 4271 基础)
// =========================================================

/// BGP 全局配置
#[derive(Debug, Clone)]
pub struct BgpGlobalConfig {
    pub local_as: u32,
    pub router_id: Ipv4Addr,
    pub listen_port: u16,
}

/// BGP 主服务结构 (Server)
/// 负责管理所有 Peer，并持有 RIB 接口
pub struct BgpServer {
    pub config: BgpGlobalConfig,
    // 邻居表: IP -> Peer 实例
    pub peers: Arc<Mutex<HashMap<IpAddr, Peer>>>,
}

impl BgpServer {
    pub fn new(config: BgpGlobalConfig) -> Self {
        Self {
            config,
            peers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 添加邻居配置
    pub async fn add_peer(&self, peer_config: PeerConfig) {
        let mut peers = self.peers.lock().await;
        let ip = peer_config.neighbor_ip;
        let peer = Peer::new(peer_config);
        peers.insert(ip, peer);
        println!("🌐 [BgpServer] 添加静态邻居: {}", ip);
    }

    /// 启动 TCP 监听器 (被动模式)
    pub async fn run_listener(&self) -> std::io::Result<()> {
        // 绑定到 BGP 标准端口 179
        // 注意：生产环境应该绑定到 0.0.0.0 或特定 IP
        let addr = format!("0.0.0.0:{}", self.config.listen_port);
        let listener = TcpListener::bind(&addr).await?;
        
        println!("👂 [BgpServer] 开始监听 BGP 端口: {}", addr);

        loop {
            // 1. 等待新的连接
            let (stream, socket_addr) = listener.accept().await?;
            let peer_ip = socket_addr.ip();
            
            println!("⚡ [BgpServer] 收到新的 TCP 连接请求来自: {}", peer_ip);

            // 2. 检查这个 IP 是不是我们的配置好的邻居
            //    (我们要锁住 peers 表进行查找)
            let mut peers = self.peers.lock().await;
            
            if let Some(peer) = peers.get_mut(&peer_ip) {
                // === 命中！这是已知的邻居 ===
                println!("✅ [BgpServer] 识别到已知邻居: {} (AS{})", 
                    peer_ip, peer.config.remote_as);

                // 3. 简单的状态流转 (Phase 2 先做最简单的)
                // 如果当前没有连接，就接受这个连接
                match peer.state {
                    SessionState::Idle | SessionState::Connect | SessionState::Active => {
                        println!("🤝 [BgpServer] 接受连接，状态迁移 -> Active (模拟)");
                        
                        // 将 TCP 流保存到 Peer 结构中
                        peer.stream = Some(stream);
                        // 更新状态 (这里暂时简略，真正 FSM 会更复杂)
                        peer.state = SessionState::Active; 
                        
                        // TODO: 在 Phase 3 这里会启动一个 tokio::spawn 来处理报文读写
                    }
                    _ => {
                        println!("⚠️ [BgpServer] 邻居 {} 已经处于连接状态，拒绝重复连接", peer_ip);
                        // stream 会在这里被 drop，连接自动断开
                    }
                }
            } else {
                // === 未知 IP，拒绝连接 ===
                println!("⛔ [BgpServer] 拒绝未知连接: {}", peer_ip);
                // stream 被 drop，连接断开
            }
        }
    }
}

// =========================================================
// 2. 上下文与主逻辑 (保留旧代码精华)
// =========================================================

/// 上下文：打包 BGP 运行所需的所有服务
pub struct BgpContext {
    pub rib: Arc<dyn RibService>,
    pub ifmgr: Arc<dyn InterfaceService>,
}

pub async fn run(ctx: BgpContext) {
    println!("🌍 BGP 组件启动 (v2.1 融合架构版)...");

    // --- 步骤 A: 初始化 BGP Server ---
    let global_config = BgpGlobalConfig {
        local_as: 64512,
        router_id: "1.1.1.1".parse().unwrap(),
        listen_port: 179,
    };
    // Server 目前只管理 Peer 状态，不直接持有 RIB/IfMgr，交互通过 ctx 进行
    let server = Arc::new(BgpServer::new(global_config));

    // (模拟) 预先添加一个邻居，为后续 FSM 开发做准备
    let peer_conf = PeerConfig {
        neighbor_ip: "192.168.10.2".parse().unwrap(),
        remote_as: 64513,
        local_as: 64512,
        router_id: "2.2.2.2".parse().unwrap(),
        hold_time: 180,
        keepalive_time: 60,
        enabled_families: vec![AfiSafi::default()],
    };
    server.add_peer(peer_conf).await;

    // 3. 【新增】启动 Listener 任务
    // 使用 server.clone() 传递给新线程
    let server_listener = server.clone();
    tokio::spawn(async move {
        if let Err(e) = server_listener.run_listener().await {
            eprintln!("🔥 BGP Listener 崩溃: {}", e);
        }
    });


    // --- 步骤 B: 接口快照 (Snapshot) [旧代码保留] ---
    // 必须在查全量之前订阅，防止漏掉
    let mut if_rx = ctx.ifmgr.subscribe();

    let current_interfaces = ctx.ifmgr.get_all_interfaces().await;
    for iface in current_interfaces {
        if iface.state == OperState::Up {
            println!("🔍 [Snapshot] BGP 发现已有接口 Up: {} -> 尝试触发 FSM", iface.name);
            // TODO: 这里未来会调用 server.check_peer_connect(iface.ip_addrs)
        }
    }

    // --- 步骤 C: 接口增量监听 (Delta) [旧代码保留] ---
    // 启动后台任务监听 Interface 变化
    let server_ref = server.clone(); // 如果监听线程需要操作 peer，可以用这个
    tokio::spawn(async move {
        println!("👂 BGP 事件监听线程已就绪...");
        loop {
            match if_rx.recv().await {
                Ok(event) => {
                    match event {
                        InterfaceEvent::LinkUp(entry) => {
                            println!("🔔 [Delta] 接口 {} Up! -> 检查是否需建立邻居", entry.name);
                            // TODO: server_ref.trigger_connection(...)
                        }
                        InterfaceEvent::LinkDown(name) => {
                            println!("🔔 [Delta] 接口 {} Down! -> 检查是否需断开邻居", name);
                        }
                        InterfaceEvent::MtuChanged(name, new_mtu) => {
                            println!("ℹ️ [Delta] 接口 {} MTU 变更为 {}", name, new_mtu);
                        }
                        _ => {}
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                    println!("⚠️ BGP 处理太慢，丢失了 {} 条广播消息", count);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    println!("🛑 广播通道已关闭");
                    break;
                }
            }
        }
    });

    // --- 步骤 D: 主循环与模拟路由 (Main Loop) [旧代码保留] ---
    println!("🚀 BGP 主路由循环启动 (模拟运行中 V2.1)...");
    tokio::time::sleep(Duration::from_secs(999999)).await; // 保持主线程不退

    /*
    // 模拟等待
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut counter = 0;
    loop {
        counter += 1;
        
        // 模拟生成一条路由
        let prefix_str = format!("10.0.{}.0/24", counter % 255);
        let prefix: IpNet = prefix_str.parse().unwrap();
        
        let entry = RouteEntry {
            protocol: RouteProtocol::BGP,
            prefix,
            nexthop: "192.168.1.1".parse().unwrap(),
            metric: 100,
            ..Default::default()
        };

        println!("⚡ [Tick {}] BGP 注入路由: {}", counter, prefix);
        
        // 调用 RIB 服务接口写入
        ctx.rib.update_route(entry).await;

        // 这里未来会加上: server.process_timers() 处理 Keepalive 定时器
        
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
    */
}