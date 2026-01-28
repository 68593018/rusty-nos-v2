use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt}; // <--- 必须加这个！
use std::io::Cursor;
use bytes::{BytesMut, BufMut}; // <--- 关键！必须加上 BufMut

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::mpsc; // 用于线程间通信
use tokio::select;     // 用于同时等待 IO 和 定时器

// 头部引入 UpdateMessage
use crate::packet::{BgpHeader, BgpMessageType, OpenMessage, UpdateMessage};

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
                        println!("🤝 [BgpServer] 接受连接，准备握手...");
                        
                        // 更新状态
                        peer.state = SessionState::Active; 
                        
                        // === 关键修改：启动握手任务 ===
                        // 我们把 stream 的所有权拿走，交给一个新的异步任务去跑
                        // 这样主线程可以继续回去监听端口
                        //let mut stream = stream;
                        let local_as = peer.config.local_as;
                        let router_id = peer.config.router_id;
                        let remote_ip = peer_ip;

                        tokio::spawn(async move {
                            if let Err(e) = handle_neighbor_session(stream, local_as, router_id).await {
                                eprintln!("🔥 [Peer {}] 会话错误: {}", remote_ip, e);
                            }
                        });
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

/// 处理单个邻居的会话逻辑
/// 包含：握手 -> 拆分读写 -> 主循环 (Select Loop)
async fn handle_neighbor_session(
    mut stream: tokio::net::TcpStream, 
    local_as: u32, 
    router_id: std::net::Ipv4Addr
) -> std::io::Result<()> {
    
    // =================================================================
    // Phase 3: 握手阶段 (Handshake) - 依然保持线性，确保建立后再进入循环
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
    
    // 这里我们将 TcpStream 所有权拿走，拆分为“读半部”和“写半部”
    // 注意：这里的 stream 变成了 owned_read 和 owned_write
    //let stream = std::mem::replace(stream, tokio::net::TcpStream::connect("0.0.0.0:0").await?); // Hack to take ownership
    let (mut reader, mut writer) = stream.into_split();

    // 创建一个通道 (Channel): 主逻辑 -> 发送任务
    // 容量 32 代表可以缓存 32 个待发送的包
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(32);

    // --- 启动后台发送任务 (Writer Task) ---
    // 它只做一件事：从通道收数据，写入 socket
    tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            if let Err(e) = writer.write_all(&packet).await {
                eprintln!("🔥 [Writer] 发送失败，连接可能已断开: {}", e);
                break;
            }
        }
        println!("👋 [Writer] 发送任务结束");
    });

    // =================================================================
    // Phase 4: 主事件循环 (Main Event Loop)
    // =================================================================
    println!("🔄 [Session] 进入全双工主循环...");
    
    // 定义定时器：Keepalive (例如每 10 秒发一次)
    let mut keepalive_timer = tokio::time::interval(std::time::Duration::from_secs(10));
    
    loop {
        // 使用 tokio::select! 宏，这就像 C语言的 select/epoll
        // 谁先准备好，就处理谁
        tokio::select! {
            // 事件 A: 收到网络数据 (Reader)
            result = read_packet(&mut reader) => {
                match result {
                    Ok(Some(msg_type)) => {
                         // 收到包的处理逻辑 (例如收到 Update)
                         // 可以在这里调用 handle_update(...)
                    }
                    Ok(None) => {
                        println!("⚠️ [Session] 对方关闭了连接");
                        break;
                    }
                    Err(e) => {
                        eprintln!("❌ [Session] 读取错误: {}", e);
                        break;
                    }
                }
            }

            // 事件 B: 定时器到期 (Timer)
            _ = keepalive_timer.tick() => {
                // 发送 Keepalive
                // 构造一个 Keepalive 包 (19字节头，无Body)
                // println!("💓 [Timer] 发送 Keepalive...");
                let mut buf = BytesMut::new();
                // 偷懒构造：16个FF + Length(19) + Type(4)
                for _ in 0..16 { buf.put_u8(0xFF); }
                buf.put_u16(19);
                buf.put_u8(4); // Type 4 = Keepalive
                
                // 通过通道发给 Writer 任务
                if tx.send(buf.to_vec()).await.is_err() {
                    break; // 通道断了说明 Writer 死了
                }
            }
            
            // 未来事件 C: 收到 RIB 的路由变动通知
            // _ = rib_rx.recv() => { ... Send Update ... }
        }
    }

    Ok(())
}

/// 辅助函数：从 Reader 读取一个完整的 BGP 包
/// 返回：Ok(Some(MsgType)) 表示成功读到一个包
async fn read_packet(reader: &mut OwnedReadHalf) -> std::io::Result<Option<BgpMessageType>> {
    // 1. 读 Header
    let mut header_buf = [0u8; 19];
    // read_exact 返回 0 表示 EOF
    match reader.read_exact(&mut header_buf).await {
        Ok(_) => {},
        Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    };

    let mut cursor = Cursor::new(&header_buf[..]);
    let header = BgpHeader::decode(&mut cursor)?;

    // 2. 读 Body
    let body_len = header.length as usize - 19;
    if body_len > 0 {
        let mut body_buf = vec![0u8; body_len];
        reader.read_exact(&mut body_buf).await?;
        
        let mut body_cursor = Cursor::new(&body_buf[..]);
        
        if header.msg_type == BgpMessageType::Update {
            // 这里解析 Update
            let update = UpdateMessage::decode(&mut body_cursor)?;
            println!("📦 [RX] UPDATE: 撤销 {:?}, 新增 {:?}", update.withdrawn_routes.len(), update.nlri.len());
        }
    } else if header.msg_type == BgpMessageType::Keepalive {
        println!("💓 [RX] Keepalive");
    }

    Ok(Some(header.msg_type))
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