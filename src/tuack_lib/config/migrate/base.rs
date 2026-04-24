use crate::prelude::*;

pub trait Migrater {
    fn migrate_contest(config: serde_json::Value) -> Result<serde_json::Value>;
    fn migrate_day(config: serde_json::Value) -> Result<serde_json::Value>;
    fn migrate_problem(config: serde_json::Value) -> Result<serde_json::Value>;
}
