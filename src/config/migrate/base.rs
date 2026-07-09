use crate::prelude::*;

pub struct MigraterMetadata {
    /// 是否强制要求迁移，比如迁移涉及文件更变等
    pub force: bool,
    /// 迁移通知信息
    pub notice: Option<&'static str>,
}

/// 数据迁移器
pub trait Migrater: Send + Sync {
    fn metadata(&self) -> MigraterMetadata;
    /// 迁移比赛层级配置
    fn migrate_contest(&self, config: serde_json::Value, dir: &Path) -> Result<serde_json::Value>;
    /// 迁移比赛日层级配置
    fn migrate_day(&self, config: serde_json::Value, dir: &Path) -> Result<serde_json::Value>;
    /// 迁移题目层级配置
    fn migrate_problem(&self, config: serde_json::Value, dir: &Path) -> Result<serde_json::Value>;
}

use std::collections::HashMap;
use std::sync::LazyLock;

pub static MIGRATERS: LazyLock<HashMap<i32, Box<dyn Migrater>>> = LazyLock::new(|| {
    let mut map = HashMap::<i32, Box<dyn Migrater>>::new();
    map.insert(3, Box::new(crate::config::migrate::v3::V3Migrater));
    map.insert(4, Box::new(crate::config::migrate::v4::V4Migrater));
    map.insert(5, Box::new(crate::config::migrate::v5::V5Migrater));
    map
});
