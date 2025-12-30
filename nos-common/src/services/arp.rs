use async_trait::async_trait;

#[async_trait]
pub trait ArpService: Send + Sync {
    // 暂时不定义任何方法
}