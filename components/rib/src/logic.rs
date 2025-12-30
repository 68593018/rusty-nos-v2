use std::sync::Arc;
use tokio::sync::{RwLock, Notify};
use crate::state::RibState;

pub struct RibLogic {
    // Logic 只持有数据的引用 (Arc)
    state: Arc<RwLock<RibState>>,
    notify: Arc<Notify>,
}

impl RibLogic {
    // 构造函数：注入 Service 的部件
    pub fn new(state: Arc<RwLock<RibState>>, notify: Arc<Notify>) -> Self {
        Self { state, notify }
    }

    // 主运行循环 (后台线程)
    pub async fn run(self) {
        println!("🧠 RIB Logic 引擎启动 (等待唤醒)...");
        
        loop {
            // 1. 挂起等待 (Wait)
            // 如果没有路由更新，这里完全不耗 CPU
            self.notify.notified().await;
            
            // 2. 醒来，获取读锁 (Read Lock)
            // 读锁是共享的，不会阻塞 BGP 写入（只要写入那一瞬间结束了）
            let guard = self.state.read().await;
            
            // 3. 执行计算逻辑 (Compute)
            println!("-----------------------------------------");
            println!("⚙️  RIB Logic 触发计算 | Ver: {}", guard.version);
            println!("   当前路由表规模: {} 条", guard.routes.len());
            
            // 打印最新的一条看看
            if let Some((prefix, entry)) = guard.routes.iter().last() {
                println!("   最新路由: {} via {}", prefix, entry.nexthop);
            }
            println!("-----------------------------------------");
            
            // guard 在这里释放
        }
    }
}