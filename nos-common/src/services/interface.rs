use async_trait::async_trait;
use tokio::sync::broadcast;
use crate::data::interface::{InterfaceEntry, InterfaceEvent};

#[async_trait]
pub trait InterfaceService: Send + Sync {
    // --- 查询类 (Pull) ---
    async fn get_interface(&self, name: &str) -> Option<InterfaceEntry>;
    async fn get_all_interfaces(&self) -> Vec<InterfaceEntry>;

    // --- 订阅类 (Push / Pub-Sub) ---
    // 不需要 async，因为只是创建一个接收端句柄
    fn subscribe(&self) -> broadcast::Receiver<InterfaceEvent>;
}