use async_trait::async_trait;

use crate::data::AsyncReader;
use crate::prelude::*;

/// 资源提供方：按"题目编号 + 逻辑路径"惰性返回资源字节流。
///
/// 使用者借此获取资源，不直接接触文件系统。
#[async_trait]
pub trait AssetProvider: Send + Sync {
    /// 获取第 `idx` 题的资源 `path`
    async fn load(&self, idx: u64, path: &Path) -> Result<Box<dyn AsyncReader>>;
}
