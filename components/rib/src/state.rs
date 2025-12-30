use std::collections::HashMap;
use ipnet::IpNet;
use nos_common::data::rib::RouteEntry;

// 1. 定义纯数据状态
// 这里只有 HashMap，没有 Mutex/RwLock，因为它被 Service 层保护
#[derive(Debug, Default)]
pub struct RibState {
    // 路由表：前缀 -> 路由条目
    pub routes: HashMap<IpNet, RouteEntry>,
    
    // 版本号：每次修改 +1
    // Logic 线程可以通过对比版本号，知道数据是否变了
    pub version: u64,
}

impl RibState {
    // 纯同步方法，极速执行
    pub fn update(&mut self, entry: RouteEntry) {
        self.routes.insert(entry.prefix, entry);
        self.version += 1;
    }
}