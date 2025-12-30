use std::time::Duration;
use ipnet::IpNet;
// 1. 只引用接口，不引用具体实现
use nos_common::services::RibService; 
// 2. 引用数据定义
use nos_common::data::rib::{RouteEntry, RouteProtocol};

// BGP 主任务
// 参数 rib: Box<dyn RibService> 表示“任何实现了 RibService 接口的对象”
pub async fn run(rib: Box<dyn RibService>) {
    println!("🌍 BGP 组件启动 (等待邻居建立)...");
    
    // 模拟 BGP 建立邻居耗时
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!("🤝 BGP Neighbor 192.168.1.2 Established!");

    let mut counter = 0;

    // 模拟持续接收路由
    loop {
        counter += 1;
        
        // 构造一个动态的路由前缀 (10.0.X.0/24)
        let prefix_str = format!("10.0.{}.0/24", counter % 255);
        let prefix: IpNet = prefix_str.parse().unwrap();

        // 构造数据对象 (使用 struct update syntax 简化代码)
        let entry = RouteEntry {
            protocol: RouteProtocol::BGP,
            prefix,
            nexthop: "192.168.1.1".parse().unwrap(),
            metric: 100,
            ..Default::default() // 其他字段用默认值
        };

        println!("⚡ [Tick {}] BGP 收到路由更新: {} -> 调用接口", counter, prefix);
        
        // 3. 核心调用：通过接口发送数据
        // BGP 根本不知道这行代码背后会触发锁、通知和后台计算
        rib.update_route(entry).await;

        // 每 3 秒产生一条
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}