use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt}; 
use std::io::Cursor;
use bytes::{BytesMut, BufMut}; 

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf}; // 注意：未使用的引用可以在后期清理
use tokio::sync::mpsc; // 用于线程间通信
use tokio::select;     // 用于同时等待 IO 和 定时器

// 引入报文结构
use crate::packet::{BgpHeader, BgpMessageType, OpenMessage, UpdateMessage};

use ipnet::IpNet;

// 引入公共定义
use nos_common::services::{RibService, InterfaceService};
use nos_common::data::rib::{RouteEntry, RouteProtocol};
use nos_common::data::interface::{InterfaceEvent, OperState};

// === 模块定义 ===
pub mod family;
pub mod packet; 
// pub mod fsm;    // 暂时注释
mod peer;

use peer::{Peer, PeerConfig};
use family::AfiSafi;

use tokio::net::TcpListener;
use crate::peer::SessionState;

// =========================================================
// 1. 数据结构定义 (RFC 4271 基础)
// =========================================================

/// BGP 全局配置
#[derive(Debug, Clone)]
pub struct BgpGlobalConfig {
    pub local_as: u32,
    pub router_id: Ipv4Addr,
    pub listen_port: u16,
}

/// 上下文：打包 BGP 运行所需的所有服务
/// ✅ 修改：增加 Clone，以便在 Listener 中传递
#[derive(Clone)] 
pub struct BgpContext {
    pub rib: Arc<dyn RibService>,
    pub ifmgr: Arc<dyn InterfaceService>,
}

/// BGP 主服务结构 (Server)
/// ✅ 修改：纯净结构，不再持有 RIB/IfMgr，完全数据驱动
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
    /// 
    /// ✅ 核心修改：接收 BgpContext 进行依赖注入
    /// Listener 本身不使用 RIB，但它负责把 Context 传给具体的会话任务
    pub async fn run_listener(&self, ctx: BgpContext) -> std::io::Result<()> {
        let addr = format!("0.0.0.0:{}", self.config.listen_port);
        let listener = TcpListener::bind(&addr).await?;
        
        println!("👂 [BgpServer] 开始监听 BGP 端口: {}", addr);

        loop {
            // 1. 等待新连接
            let (stream, socket_addr) = listener.accept().await?;
            let peer_ip = socket_addr.ip();
            
            println!("⚡ [BgpServer] 收到新的 TCP 连接请求来自: {}", peer_ip);

            // 2. 检查是否为已知邻居
            let mut peers = self.peers.lock().await;
            
            if let Some(peer) = peers.get_mut(&peer_ip) {
                // === 命中！===
                println!("✅ [BgpServer] 识别到已知邻居: {} (AS{})", 
                    peer_ip, peer.config.remote_as);

                // 3. 状态检查与任务分发
                match peer.state {
                    SessionState::Idle | SessionState::Connect | SessionState::Active => {
                        println!("🤝 [BgpServer] 接受连接，准备握手...");
                        
                        // 更新状态
                        peer.state = SessionState::Active; 
                        
                        // 准备参数
                        let stream = stream;
                        let local_as = peer.config.local_as;
                        let router_id = peer.config.router_id;
                        let remote_ip = peer_ip;

                        // === 关键点：从 Context 中提取 RIB 服务句柄 ===
                        // 这里完成了“依赖注入”的最后一步
                        let rib_handle = ctx.rib.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_neighbor_session(stream, local_as, router_id, rib_handle).await {
                                eprintln!("🔥 [Peer {}] 会话错误: {}", remote_ip, e);
                            }
                        });
                    }
                    _ => {
                        println!("⚠️ [BgpServer] 邻居 {} 已经处于连接状态，拒绝重复连接", peer_ip);
                    }
                }
            } else {
                // === 未知 IP ===
                println!("⛔ [BgpServer] 拒绝未知连接: {}", peer_ip);
            }
        }
    }
}

/// 内部事件枚举：用于统一处理网络包类型
enum BgpEvent {
    Keepalive,
    Update(UpdateMessage),
    Notification,
    None, // 连接断开
}

/// 处理单个邻居的会话逻辑
/// 流程：握手 -> 拆分读写 -> 主循环 (注入路由到 RIB)
async fn handle_neighbor_session(
    mut stream: tokio::net::TcpStream, 
    local_as: u32, 
    router_id: std::net::Ipv4Addr,
    rib: Arc<dyn RibService> // <--- 注入进来的 RIB 服务
) -> std::io::Result<()> {
    
    // =================================================================
    // Phase 3: 握手阶段 (Handshake)
    // =================================================================
    
    // 1. 发送 OPEN
    println!("📤 [Out] 发送 OPEN 消息...");
    let open_msg = OpenMessage::new(local_as as u16, 180, router_id);
    let mut body_buf = BytesMut::new();
    open_msg.encode(&mut body_buf);
    
    let header = BgpHeader {
        length: (BgpHeader::LENGTH + body_buf.len()) as u16,
        msg_type: BgpMessageType::Open,
    };
    let mut final_buf = BytesMut::new();
    header.encode(&mut final_buf);
    final_buf.extend_from_slice(&body_buf);
    stream.write_all(&final_buf).await?;

    // 2. 接收 OPEN
    println!("📥 [In] 等待对方 OPEN 消息...");
    let mut header_buf = [0u8; 19];
    stream.read_exact(&mut header_buf).await?;
    let mut cursor = Cursor::new(&header_buf[..]);
    let header = BgpHeader::decode(&mut cursor)?;
    
    if header.msg_type != BgpMessageType::Open {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Expected OPEN"));
    }

    let body_len = header.length as usize - 19;
    let mut body_buf = vec![0u8; body_len];
    stream.read_exact(&mut body_buf).await?;
    let mut body_cursor = Cursor::new(&body_buf[..]);
    let received_open = OpenMessage::decode(&mut body_cursor)?;

    println!("✅ [Handshake] 握手成功! Peer AS: {}", received_open.my_as);

    // =================================================================
    // Phase 4: 拆分读写流 (Split Stream)
    // =================================================================
    
    let (mut reader, mut writer) = stream.into_split();

    // 创建发送通道
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);

    // --- 后台发送任务 (Writer Task) ---
    tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            if let Err(e) = writer.write_all(&packet).await {
                eprintln!("🔥 [Writer] 发送失败: {}", e);
                break;
            }
        }
        println!("👋 [Writer] 发送任务结束");
    });

    // =================================================================
    // Phase 5: 主事件循环 (决策与注入)
    // =================================================================
    println!("🔄 [Session] 进入全双工主循环 (Ready to Inject)...");
    
    let mut keepalive_timer = tokio::time::interval(std::time::Duration::from_secs(10));
    
    loop {
        tokio::select! {
            // 事件 A: 收到网络包
            event = read_packet_full(&mut reader) => {
                match event {
                    Ok(BgpEvent::Update(update)) => {
                        // === 核心逻辑：路由注入 ===
                        println!("📦 [RX] UPDATE: 新增 {} 条路由", update.nlri.len());
                        
                        // 1. 提取下一跳 (Next Hop)
                        if let Some(nexthop_ip) = extract_nexthop(&update.attributes) {
                            // 2. 遍历 NLRI 并注入 RIB
                            for prefix in update.nlri {
                                let entry = RouteEntry {
                                    protocol: RouteProtocol::BGP,
                                    prefix: IpNet::V4(prefix),
                                    nexthop: nexthop_ip,
                                    metric: 0,
                                    ..Default::default()
                                };
                                
                                println!("   🚀 [Inject] 注入路由: {} via {}", prefix, nexthop_ip);
                                // 3. 调用 RIB 服务接口
                                rib.update_route(entry).await;
                            }
                        } else {
                            if !update.nlri.is_empty() {
                                eprintln!("   ⚠️ 忽略更新：未找到 NEXT_HOP 属性");
                            }
                        }
                    }
                    Ok(BgpEvent::Keepalive) => { /* 忽略心跳 */ }
                    Ok(BgpEvent::Notification) => { break; }
                    Ok(BgpEvent::None) => { 
                        println!("⚠️ [Session] 对方关闭了连接");
                        break; 
                    }
                    Err(e) => {
                        eprintln!("❌ [Session] 读取错误: {}", e);
                        break;
                    }
                }
            }

            // 事件 B: Keepalive 定时器
            _ = keepalive_timer.tick() => {
                let mut buf = BytesMut::new();
                for _ in 0..16 { buf.put_u8(0xFF); }
                buf.put_u16(19);
                buf.put_u8(4); // Type 4 = Keepalive
                
                if tx.send(buf.to_vec()).await.is_err() {
                    break;
                }
            }
        }
    }

    Ok(())
}

/// 辅助函数：从属性中提取 Next Hop IP
fn extract_nexthop(attrs: &[crate::packet::attribute::Attribute]) -> Option<std::net::IpAddr> {
    for attr in attrs {
        // AttributeTypeCode::NextHop = 3
        if attr.type_code == 3 {
            // NextHop 必须是 4 字节 IPv4
            if attr.value.len() == 4 {
                let ip = std::net::Ipv4Addr::new(
                    attr.value[0], attr.value[1], attr.value[2], attr.value[3]
                );
                return Some(std::net::IpAddr::V4(ip));
            }
        }
    }
    None
}

/// 辅助函数：读取完整包并转换为 BgpEvent
async fn read_packet_full(reader: &mut OwnedReadHalf) -> std::io::Result<BgpEvent> {
    // 读头
    let mut header_buf = [0u8; 19];
    match reader.read_exact(&mut header_buf).await {
        Ok(_) => {},
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(BgpEvent::None),
        Err(e) => return Err(e),
    };

    let mut cursor = Cursor::new(&header_buf[..]);
    let header = BgpHeader::decode(&mut cursor)?;

    // 读体
    let body_len = header.length as usize - 19;
    let mut body_buf = vec![0u8; body_len];
    if body_len > 0 {
        reader.read_exact(&mut body_buf).await?;
    }
    let mut body_cursor = Cursor::new(&body_buf[..]);

    match header.msg_type {
        BgpMessageType::Update => {
            let update = UpdateMessage::decode(&mut body_cursor)?;
            Ok(BgpEvent::Update(update))
        }
        BgpMessageType::Keepalive => Ok(BgpEvent::Keepalive),
        BgpMessageType::Notification => Ok(BgpEvent::Notification),
        _ => Ok(BgpEvent::Keepalive), // 忽略其他类型
    }
}

// =========================================================
// 2. 主逻辑入口 (Ctx 组装)
// =========================================================

pub async fn run(ctx: BgpContext) {
    println!("🌍 BGP 组件启动 (v2.2 数据驱动版)...");

    // --- 步骤 A: 初始化 BGP Server ---
    let global_config = BgpGlobalConfig {
        local_as: 64512,
        router_id: "1.1.1.1".parse().unwrap(),
        listen_port: 179,
    };
    // Server 纯净初始化，不持有 RIB
    let server = Arc::new(BgpServer::new(global_config));

    // 添加静态邻居
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

    // --- 步骤 B: 启动 Listener ---
    let server_listener = server.clone();
    
    // ✅ 关键：克隆 Context 传递给 Listener 任务
    let ctx_for_listener = ctx.clone();

    tokio::spawn(async move {
        // 注入 Context
        if let Err(e) = server_listener.run_listener(ctx_for_listener).await {
            eprintln!("🔥 BGP Listener 崩溃: {}", e);
        }
    });

    // --- 步骤 C: 接口监控 (保持原逻辑) ---
    let mut if_rx = ctx.ifmgr.subscribe();

    tokio::spawn(async move {
        println!("👂 BGP 事件监听线程已就绪...");
        loop {
            match if_rx.recv().await {
                Ok(event) => {
                    match event {
                        InterfaceEvent::LinkUp(entry) => {
                            println!("🔔 [Delta] 接口 {} Up! -> 检查是否需建立邻居", entry.name);
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

    println!("🚀 BGP 主路由循环启动 (正式版 V2.2)...");
    tokio::time::sleep(Duration::from_secs(999999)).await; 
}