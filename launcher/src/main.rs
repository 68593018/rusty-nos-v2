use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("==============================================");
    println!("🚀 RustyNOS v2 启动 (状态逻辑分离架构)");
    println!("==============================================");

    // 1. 【创建稳态】实例化 RIB Service
    // 这是一个线程安全的容器，持有数据 (RwLock) 和通知器 (Notify)
    let rib_service = comp_rib::RibServiceConcrete::new();

    // 2. 【创建敏态】实例化 RIB Logic
    // 我们从 service 中克隆出 state 和 notify 的引用 (Arc) 注入给 Logic
    // 这样 Logic 就能感知 Service 的变化
    let rib_logic = comp_rib::RibLogic::new(
        rib_service.state.clone(),
        rib_service.notify.clone()
    );

    // 3. 【启动后台计算】
    // 这是一个死循环任务，负责处理繁重的计算
    tokio::spawn(async move {
        rib_logic.run().await;
    });

    // 4. 【依赖注入】启动 BGP
    // 关键步骤：我们将具体的 rib_service 包装成抽象的 Box<dyn RibService>
    // 这样 BGP 就只能看到接口，看不到内部实现
    let rib_interface = Box::new(rib_service);
    
    tokio::spawn(async move {
        comp_bgp::run(rib_interface).await;
    });

    // 5. 阻止主线程退出
    println!("✅ 系统就绪，按 Ctrl+C 退出...\n");
    signal::ctrl_c().await?;
    println!("\n🛑 系统关闭");
    
    Ok(())
}