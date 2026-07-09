use crate::prelude::*;

pub struct V4Migrater;

impl super::base::Migrater for V4Migrater {
    fn metadata(&self) -> super::base::MigraterMetadata {
        super::base::MigraterMetadata {
            force: true,
            notice: Some(
                r#"您可能需要自行配置 generator 的依赖内容，比如 `"deps": [ "testlib.h" ]`"#,
            ),
        }
    }

    fn migrate_contest(
        &self,
        mut config: serde_json::Value,
        _dir: &Path,
    ) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(5));
        }
        Ok(config)
    }

    fn migrate_day(&self, mut config: serde_json::Value, _dir: &Path) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(5));
        }
        Ok(config)
    }

    fn migrate_problem(
        &self,
        mut config: serde_json::Value,
        dir: &Path,
    ) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(5));
        }

        if dir.join("gen/gen.cpp").exists() {
            let data = serde_json::json!({
                "gen": "gen/gen.cpp",
                "deps": []
            });

            let sample = if dir.join("gen/gen_sample.cpp").exists() {
                serde_json::json!({
                    "gen": "gen/gen_sample.cpp",
                    "deps": []
                })
            } else {
                serde_json::Value::Null
            };

            obj.insert(
                "generator".to_string(),
                serde_json::json!({
                    "data": data,
                    "sample": sample,
                }),
            );
        }

        Ok(config)
    }
}
