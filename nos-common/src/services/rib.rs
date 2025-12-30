use async_trait::async_trait;
// 【重要】引用 data 目录下的对应结构体
use crate::data::rib::RouteEntry;

#[async_trait]
pub trait RibService: Send + Sync {
    async fn update_route(&self, entry: RouteEntry);
}