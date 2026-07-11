use super::base::*;
use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum DataItem {
    /// 单个对象
    Single(SingleDataItem),
    /// 组合对象
    Bundle(BundleDataItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SingleDataItem {
    /// 测试点编号
    pub id: u32,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号
    #[serde(default)]
    pub subtask: u32,
    /// 输入文件
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub input: Option<String>,
    /// 输出文件
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub output: Option<String>,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点（旧格式，仅反序列化）
    #[serde(default, skip_serializing)]
    pub manual: Option<bool>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BundleDataItem {
    /// 测试点编号
    pub id: Vec<i32>,
    /// 测试点分值
    pub score: u32,
    /// Subtask 编号
    #[serde(default)]
    pub subtask: u32,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点（旧格式，仅反序列化）
    #[serde(default, skip_serializing)]
    pub manual: Option<bool>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SampleItem {
    /// 样例编号
    pub id: u32,
    /// 输入文件
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    /// 输出文件
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// 参数，会从全局参数继承
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub args: HashMap<String, i64>,
    /// 是否为人工生成的测试点（旧格式，仅反序列化）
    #[serde(default, skip_serializing)]
    pub manual: Option<bool>,
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<DmkConfig>,
}

#[allow(unused)]
pub struct V3Migrater;

impl Migrater for V3Migrater {
    fn metadata(&self) -> MigraterMetadata {
        MigraterMetadata {
            force: false,
            notice: None,
        }
    }

    fn migrate_contest(
        &self,
        mut config: serde_json::Value,
        _dir: &Path,
    ) -> Result<serde_json::Value> {
        match config.as_object_mut() {
            Some(obj) => {
                if obj.get("version").and_then(|v| v.as_u64()).is_some() {
                    obj.insert("version".to_string(), serde_json::json!(4));
                }
            }
            None => {
                bail!("配置无效");
            }
        }

        Ok(config)
    }
    fn migrate_day(&self, mut config: serde_json::Value, _dir: &Path) -> Result<serde_json::Value> {
        match config.as_object_mut() {
            Some(obj) => {
                if obj.get("version").and_then(|v| v.as_u64()).is_some() {
                    obj.insert("version".to_string(), serde_json::json!(4));
                }
            }
            None => {
                bail!("配置无效");
            }
        }

        Ok(config)
    }
    fn migrate_problem(
        &self,
        mut config: serde_json::Value,
        _dir: &Path,
    ) -> Result<serde_json::Value> {
        match config.as_object_mut() {
            Some(obj) => {
                if obj.get("version").and_then(|v| v.as_u64()).is_some() {
                    obj.insert("version".to_string(), serde_json::json!(4));
                }

                if let Some(array) = obj.get("samples") {
                    let mut items: Vec<SampleItem> =
                        serde_json::from_value(array.clone()).context("配置无效")?;

                    for item in &mut items {
                        item.dmk = item
                            .manual
                            .map(|m| if m { DmkConfig::Skip } else { DmkConfig::On });
                    }

                    obj.insert("samples".to_string(), serde_json::to_value(items)?);
                }

                if let Some(array) = obj.get("data") {
                    let mut items: Vec<DataItem> =
                        serde_json::from_value(array.clone()).context("配置无效")?;

                    for item in &mut items {
                        match item {
                            DataItem::Single(s) => {
                                s.dmk = s
                                    .manual
                                    .map(|m| if m { DmkConfig::Skip } else { DmkConfig::On });
                            }
                            DataItem::Bundle(b) => {
                                b.dmk = b
                                    .manual
                                    .map(|m| if m { DmkConfig::Skip } else { DmkConfig::On });
                            }
                        }
                    }

                    obj.insert("data".to_string(), serde_json::to_value(items)?);
                }

                obj.insert("dmk".to_string(), serde_json::to_value(DmkConfig::On)?);
            }
            None => {
                bail!("配置无效");
            }
        }

        Ok(config)
    }
}
