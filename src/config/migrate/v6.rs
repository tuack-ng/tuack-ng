use crate::prelude::*;

pub struct V6Migrater;

impl super::base::Migrater for V6Migrater {
    fn metadata(&self) -> super::base::MigraterMetadata {
        super::base::MigraterMetadata {
            force: false,
            notice: None,
        }
    }

    fn migrate_contest(
        &self,
        mut config: serde_json::Value,
        _dir: &Path,
    ) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(7));
        }
        Ok(config)
    }

    fn migrate_day(&self, mut config: serde_json::Value, _dir: &Path) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(7));
        }
        Ok(config)
    }

    fn migrate_problem(
        &self,
        mut config: serde_json::Value,
        _dir: &Path,
    ) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(7));
        }
        obj.remove("partial-score");
        Ok(config)
    }
}
