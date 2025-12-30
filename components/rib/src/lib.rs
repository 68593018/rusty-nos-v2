// 声明子模块
pub mod state;
pub mod service;
pub mod logic;

// 方便外部 (Launcher) 调用
pub use service::RibServiceConcrete;
pub use logic::RibLogic;