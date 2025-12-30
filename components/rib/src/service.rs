use std::sync::Arc;
use tokio::sync::{RwLock, Notify};
use async_trait::async_trait;

// 引入公共契约
use nos_common::services::RibService;
use nos_common::data::rib::RouteEntry;

// 引入内部状态
use crate::state::RibState;

// Service 结构体：它是“稳态”，几乎不崩溃
#[derive(Clone)]
pub struct RibServiceConcrete {
    // 1. 数据容器：使用 RwLock 读写锁保护
    pub state: Arc<RwLock<RibState>>,
    
    // 2. 通知信号：用于唤醒 Logic 线程
    pub notify: Arc<Notify>,
}

impl RibServiceConcrete {
    // 初始化函数
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(RibState::default())),
            notify: Arc::new(Notify::new()),
        }
    }
}

// 实现公共接口 Trait
#[async_trait]
impl RibService for RibServiceConcrete {
    async fn update_route(&self, entry: RouteEntry) {
        {
            // A. 获取写锁 (Write Lock)
            // 这是一个极小的临界区，写入速度纳秒级
            let mut guard = self.state.write().await;
            guard.update(entry);
        } // B. 锁在这里自动释放 (Scope End)

        // C. 发送通知：告诉 Logic "来活了"
        self.notify.notify_one();
    }
}