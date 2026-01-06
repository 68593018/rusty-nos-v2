use std::sync::Arc;
use tokio::signal;
//use console_subscriber::ConsoleLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    // ==========================================================
    // 0. 【核心步骤】初始化 Console
    // ==========================================================
    // 这行代码会启动一个后台 Server，默认监听 127.0.0.1:6669
    // 它会拦截所有的 tokio::spawn 创建的任务信息
    //console_subscriber::init();


    //println!("✨ 系统运行中... (正在等待连接)");


    println!("==============================================");
    println!("🚀 RustyNOS v2.1 全系统启动 (Pub/Sub + Context)");
    println!("==============================================");

    // ==========================================
    // 1. 初始化 RIB 组件 (核心状态)
    // ==========================================
    // 创建实例
    let rib_impl = comp_rib::RibServiceConcrete::new();
    
    // 实例化 Logic (需要借用 Service 内部的 state 和 notify)
    let rib_logic = comp_rib::RibLogic::new(
        rib_impl.state.clone(),
        rib_impl.notify.clone()
    );
    
    // 用 Arc 包裹 Service，因为 BGP 需要 Arc<dyn RibService>
    // 这一步之后，rib_service 可以在多个线程间廉价克隆
    let rib_service = Arc::new(rib_impl);

    // 启动 RIB 后台计算
    tokio::spawn(async move {
        rib_logic.run().await;
    });
    println!("✅ RIB 组件就绪");

    // ==========================================
    // 2. 初始化 IfMgr 组件 (接口管理)
    // ==========================================
    // 创建实例
    let ifmgr_impl = comp_ifmgr::IfMgrServiceConcrete::new();
    
    // 用 Arc 包裹 Service
    let ifmgr_service = Arc::new(ifmgr_impl);

    // 实例化 Logic (模拟内核)
    // Logic 需要持有 Service 的引用来触发 on_link_up
    let ifmgr_logic = comp_ifmgr::IfMgrLogic::new(ifmgr_service.clone());

    // 启动 IfMgr 模拟器
    tokio::spawn(async move {
        ifmgr_logic.run().await;
    });
    println!("✅ IfMgr 组件就绪 (模拟内核已启动)");

    // ==========================================
    // 3. 初始化 BGP 组件 (业务层)
    // ==========================================
    // 依赖注入：打包所有需要的服务
    // Rust 会自动将 Arc<RibServiceConcrete> 转换为 Arc<dyn RibService>
    let bgp_ctx = comp_bgp::BgpContext {
        rib: rib_service.clone(),
        ifmgr: ifmgr_service.clone(),
    };

    // 启动 BGP 主任务
    tokio::spawn(async move {
        comp_bgp::run(bgp_ctx).await;
    });
    println!("✅ BGP 组件启动");

    // ==========================================
    // 4. 系统守候
    // ==========================================
    println!("\n✨ 系统运行中... (按 Ctrl+C 退出)\n");
    signal::ctrl_c().await?;
    println!("\n🛑 System Shutdown");
    
    Ok(())
}