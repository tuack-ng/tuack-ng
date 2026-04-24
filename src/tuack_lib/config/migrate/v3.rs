use super::base::*;
use crate::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum OldDataItem {
    /// 单个对象
    Single(OldSingleDataItem),
    /// 组合对象
    Bundle(OldBundleDataItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldSingleDataItem {
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
    /// 是否为人工生成的测试点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldBundleDataItem {
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
    /// 是否为人工生成的测试点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OldSampleItem {
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
    /// 是否为人工生成的测试点
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
#[serde(rename_all = "kebab-case")]
enum NewDmkConfig {
    /// 忽略
    Skip,
    /// 只生成输入
    Input,
    /// 只生成输出
    Output,
    /// 启用
    On,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum NewDataItem {
    /// 单个对象
    Single(NewSingleDataItem),
    /// 组合对象
    Bundle(NewBundleDataItem),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NewSingleDataItem {
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
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<NewDmkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NewBundleDataItem {
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
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<NewDmkConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NewSampleItem {
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
    /// 数据生成行为
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dmk: Option<NewDmkConfig>,
}

#[allow(unused)]
pub struct V3Migrater;

impl Migrater for V3Migrater {
    fn migrate_contest(mut config: serde_json::Value) -> Result<serde_json::Value> {
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
    fn migrate_day(mut config: serde_json::Value) -> Result<serde_json::Value> {
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
    fn migrate_problem(mut config: serde_json::Value) -> Result<serde_json::Value> {
        match config.as_object_mut() {
            Some(obj) => {
                if obj.get("version").and_then(|v| v.as_u64()).is_some() {
                    obj.insert("version".to_string(), serde_json::json!(4));
                }

                // 迁移 samples
                if let Some(array) = obj.get("samples") {
                    let old_array: Vec<OldSampleItem> =
                        serde_json::from_value(array.clone()).context("配置无效")?;

                    let new_array: Vec<NewSampleItem> = old_array
                        .into_iter()
                        .map(|v| NewSampleItem {
                            id: v.id,
                            input: v.input,
                            output: v.output,
                            args: v.args,
                            dmk: v.manual.map(|manual| {
                                if manual {
                                    NewDmkConfig::On
                                } else {
                                    NewDmkConfig::Skip
                                }
                            }),
                        })
                        .collect();

                    obj.insert("samples".to_string(), serde_json::to_value(new_array)?);
                }

                // 迁移 data
                if let Some(array) = obj.get("data") {
                    let old_array: Vec<OldDataItem> =
                        serde_json::from_value(array.clone()).context("配置无效")?;

                    let new_array: Vec<NewDataItem> = old_array
                        .into_iter()
                        .map(|v| match v {
                            OldDataItem::Single(val) => NewDataItem::Single(NewSingleDataItem {
                                id: val.id,
                                score: val.score,
                                subtask: val.subtask,
                                input: val.input,
                                output: val.output,
                                args: val.args,
                                dmk: val.manual.map(|manual| {
                                    if manual {
                                        NewDmkConfig::On
                                    } else {
                                        NewDmkConfig::Skip
                                    }
                                }),
                            }),
                            OldDataItem::Bundle(val) => NewDataItem::Bundle(NewBundleDataItem {
                                id: val.id,
                                score: val.score,
                                subtask: val.subtask,
                                args: val.args,
                                dmk: val.manual.map(|manual| {
                                    if manual {
                                        NewDmkConfig::On
                                    } else {
                                        NewDmkConfig::Skip
                                    }
                                }),
                            }),
                        })
                        .collect();

                    obj.insert("data".to_string(), serde_json::to_value(new_array)?);
                }

                obj.insert("dmk".to_string(), serde_json::to_value(NewDmkConfig::On)?);
            }
            None => {
                bail!("配置无效");
            }
        }

        Ok(config)
    }
}
