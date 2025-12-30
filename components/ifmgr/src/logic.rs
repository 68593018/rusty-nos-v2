use std::sync::Arc;
use std::time::Duration;
use nos_common::data::interface::{InterfaceEntry, OperState};
use nos_common::data::primitives::MacAddress;
use crate::service::IfMgrServiceConcrete;

pub struct IfMgrLogic {
    // Logic 直接持有 Concrete 类型，以便调用 on_link_up 等内部方法
    service: Arc<IfMgrServiceConcrete>,
}

impl IfMgrLogic {
    pub fn new(service: Arc<IfMgrServiceConcrete>) -> Self {
        Self { service }
    }

    pub async fn run(self) {
        println!("🔌 IfMgr Logic (Kernel Simulator) 启动...");
        
        // 模拟等待系统启动
        tokio::time::sleep(Duration::from_secs(2)).await;

        // --- 模拟事件 1: eth0 Up ---
        let eth0 = InterfaceEntry {
            name: "eth0".to_string(),
            ifindex: 10,
            state: OperState::Up,
            mtu: 1500,
            mac: MacAddress("aa:bb:cc:dd:ee:01".to_string()),
        };
        
        self.service.on_link_up(eth0).await;

        // 模拟等待
        tokio::time::sleep(Duration::from_secs(5)).await;

        // --- 模拟事件 2: eth1 Up ---
        let eth1 = InterfaceEntry {
            name: "eth1".to_string(),
            ifindex: 11,
            state: OperState::Up,
            mtu: 9000, // Jumbo Frame
            mac: MacAddress("aa:bb:cc:dd:ee:02".to_string()),
        };
        self.service.on_link_up(eth1).await;
        
        // 保持运行，防止 Task 退出
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}