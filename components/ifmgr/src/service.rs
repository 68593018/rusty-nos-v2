use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use async_trait::async_trait;

use nos_common::services::InterfaceService;
use nos_common::data::interface::{InterfaceEntry, InterfaceEvent};
use crate::state::IfMgrState;

pub struct IfMgrServiceConcrete {
    // 1. 状态存储 (RwLock 保护)
    pub state: Arc<RwLock<IfMgrState>>,
    
    // 2. 广播发射端 (Sender)
    // 订阅者通过 subscribe() 获取对应的 Receiver
    tx: broadcast::Sender<InterfaceEvent>,
}

impl IfMgrServiceConcrete {
    pub fn new() -> Self {
        // 创建广播通道，容量 100
        // 如果订阅者处理太慢，会收到 Lagged 错误
        let (tx, _rx) = broadcast::channel(100);
        
        Self {
            state: Arc::new(RwLock::new(IfMgrState::default())),
            tx,
        }
    }

    // --- 内部方法：供 Logic 层调用 (模拟内核上报) ---
    pub async fn on_link_up(&self, entry: InterfaceEntry) {
        // 1. 更新内存状态
        {
            let mut guard = self.state.write().await;
            guard.update(entry.clone());
        } // 锁释放

        // 2. 发送广播
        // send 返回接收者数量，我们这里忽略它
        println!("📢 IfMgr: 广播 LinkUp 事件 -> {}", entry.name);
        let _ = self.tx.send(InterfaceEvent::LinkUp(entry));
    }
}

// --- 实现对外公共接口 ---
#[async_trait]
impl InterfaceService for IfMgrServiceConcrete {
    // 订阅接口：像订阅报纸一样简单
    fn subscribe(&self) -> broadcast::Receiver<InterfaceEvent> {
        self.tx.subscribe()
    }

    async fn get_interface(&self, name: &str) -> Option<InterfaceEntry> {
        let guard = self.state.read().await;
        guard.interfaces.get(name).cloned()
    }

    async fn get_all_interfaces(&self) -> Vec<InterfaceEntry> {
        let guard = self.state.read().await;
        guard.interfaces.values().cloned().collect()
    }
}