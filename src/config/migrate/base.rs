use crate::prelude::*;

/// 数据迁移器
pub trait Migrater {
    /// 迁移比赛层级配置
    fn migrate_contest(config: serde_json::Value) -> Result<serde_json::Value>;
    /// 迁移比赛日层级配置
    fn migrate_day(config: serde_json::Value) -> Result<serde_json::Value>;
    /// 迁移题目层级配置
    fn migrate_problem(config: serde_json::Value) -> Result<serde_json::Value>;
}
