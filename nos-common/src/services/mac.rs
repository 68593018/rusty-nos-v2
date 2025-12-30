use async_trait::async_trait;

// 定义一个空的 Trait，先占个坑
#[async_trait]
pub trait MacService: Send + Sync {
    // 暂时不定义任何方法
}