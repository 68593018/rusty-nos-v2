use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use ipnet::IpNet;

// 引用公共定义
use nos_common::services::{RibService, InterfaceService};
use nos_common::data::rib::{RouteEntry, RouteProtocol};
use nos_common::data::interface::{InterfaceEvent, OperState};

// 1. 定义上下文：打包 BGP 运行所需的所有服务
// 使用 Arc 是为了方便 Clone 后在多个 Task 中共享
pub struct BgpContext {
    pub rib: Arc<dyn RibService>,
    pub ifmgr: Arc<dyn InterfaceService>,
}

pub async fn run(ctx: BgpContext) {
    println!("🌍 BGP 组件启动...");

    // =====================================================
    // 核心逻辑：快照 (Snapshot) + 增量 (Delta)
    // =====================================================

    // 1. 【订阅】先拿到接收端 (防止漏掉快照过程中的事件)
    // 必须在查全量之前订阅，否则会产生“时间黑洞”
    let mut if_rx = ctx.ifmgr.subscribe();

    // 2. 【快照】获取当前所有 Up 的接口
    // BGP 刚启动时，接口可能已经 Up 很久了，不会有广播事件，必须主动查
    let current_interfaces = ctx.ifmgr.get_all_interfaces().await;
    for iface in current_interfaces {
        if iface.state == OperState::Up {
            println!("🔍 [Snapshot] BGP 发现已有接口 Up: {} -> 尝试建立邻居", iface.name);
            // TODO: 这里调用 neighbor_fsm.start(iface)
        }
    }

    // 3. 【增量】启动后台任务监听后续变化
    // 使用 move 关键字将 rx 的所有权转移给新线程
    tokio::spawn(async move {
        println!("👂 BGP 事件监听线程已就绪...");
        loop {
            // recv() 会挂起等待，不消耗 CPU
            match if_rx.recv().await {
                Ok(event) => {
                    match event {
                        InterfaceEvent::LinkUp(entry) => {
                            println!("🔔 [Delta] BGP 收到通知: 接口 {} Up! -> 建立邻居", entry.name);
                        }
                        InterfaceEvent::LinkDown(name) => {
                            println!("🔔 [Delta] BGP 收到通知: 接口 {} Down! -> 断开邻居", name);
                        }
                        InterfaceEvent::MtuChanged(name, new_mtu) => {
                            println!("ℹ️ [Delta] 接口 {} MTU 变更为 {}", name, new_mtu);
                        }
                        _ => {}
                    }
                }
                Err(broadcast::error::RecvError::Lagged(count)) => {
                    println!("⚠️ BGP 处理太慢，丢失了 {} 条广播消息 (Lagged)", count);
                    // 生产环境中，这里通常需要重新执行一次“快照”流程
                }
                Err(broadcast::error::RecvError::Closed) => {
                    println!("🛑 广播通道已关闭");
                    break;
                }
            }
        }
    });

    // =====================================================
    // BGP 主逻辑 (模拟路由生成)
    // =====================================================
    println!("🚀 BGP 主路由循环启动 (Wait for neighbors)...");
    
    // 模拟等待邻居建立
    tokio::time::sleep(Duration::from_secs(2)).await;

    let mut counter = 0;
    loop {
        counter += 1;
        
        let prefix: IpNet = format!("10.0.{}.0/24", counter % 255).parse().unwrap();
        let entry = RouteEntry {
            protocol: RouteProtocol::BGP,
            prefix,
            nexthop: "192.168.1.1".parse().unwrap(),
            metric: 100,
            ..Default::default()
        };

        println!("⚡ [Tick {}] BGP 注入路由: {}", counter, prefix);
        ctx.rib.update_route(entry).await;

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}