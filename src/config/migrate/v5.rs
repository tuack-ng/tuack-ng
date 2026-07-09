use crate::prelude::*;

pub struct V5Migrater;

impl super::base::Migrater for V5Migrater {
    fn metadata(&self) -> super::base::MigraterMetadata {
        super::base::MigraterMetadata {
            force: true,
            notice: Some(
                r#"旧版 `use-chk` 字段已迁移为 `checker` 配置对象。您可能需要自行配置 checker 的依赖内容，比如 `"deps": [ "testlib.h" ]`"#,
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
            obj.insert("version".to_string(), serde_json::json!(6));
        }
        Ok(config)
    }

    fn migrate_day(&self, mut config: serde_json::Value, _dir: &Path) -> Result<serde_json::Value> {
        let obj = config.as_object_mut().context("配置无效")?;
        if obj.get("version").and_then(|v| v.as_u64()).is_some() {
            obj.insert("version".to_string(), serde_json::json!(6));
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
            obj.insert("version".to_string(), serde_json::json!(6));
        }

        let use_chk = obj
            .get("use-chk")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if use_chk && dir.join("data/chk/chk.cpp").exists() {
            let data = serde_json::json!({
                "source": "data/chk/chk.cpp",
                "deps": []
            });

            let sample = if dir.join("data/chk/chk_sample.cpp").exists() {
                serde_json::json!({
                    "source": "data/chk/chk_sample.cpp",
                    "deps": []
                })
            } else {
                serde_json::Value::Null
            };

            obj.insert(
                "checker".to_string(),
                serde_json::json!({
                    "data": data,
                    "sample": sample,
                }),
            );
        }

        obj.remove("use-chk");

        Ok(config)
    }
}
