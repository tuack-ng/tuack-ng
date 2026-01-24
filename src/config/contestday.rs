use super::problem::ProblemConfig;
use indexmap::IndexMap;
use log::error;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestDayConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    pub compile: HashMap<String, String>,
    #[serde(rename = "start time")]
    pub start_time: [u32; 6],
    #[serde(rename = "end time")]
    pub end_time: [u32; 6],
    #[serde(rename = "use-pretest")]
    #[serde(default)]
    pub use_pretest: Option<bool>,
    #[serde(rename = "noi-style")]
    #[serde(default)]
    pub noi_style: Option<bool>,
    #[serde(rename = "file-io")]
    #[serde(default)]
    pub file_io: Option<bool>,
    #[serde(skip)]
    // pub subconfig: Vec<ProblemConfig>,
    pub subconfig: IndexMap<String, ProblemConfig>,
    #[serde(skip)]
    pub path: PathBuf,
}

/// 加载比赛日配置
pub fn load_day_config(
    dayconfig_path: &Path,
) -> Result<ContestDayConfig, Box<dyn std::error::Error>> {
    // 读取并验证每日配置文件
    let day_content = fs::read_to_string(dayconfig_path)?;
    let day_json_value: serde_json::Value = serde_json::from_str(&day_content)?;

    // 检查版本
    if let Some(version) = day_json_value.get("version").and_then(|v| v.as_u64())
        && version < 3
    {
        error!("配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。");
        return Err("配置文件版本过低".into());
    }

    let mut dayconfig: ContestDayConfig = serde_json::from_str(&day_content)?;

    dayconfig.path = dayconfig_path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("无法获取配置文件父目录")?;

    // 不处理子目录，只加载当前配置
    dayconfig.subconfig = IndexMap::new();

    Ok(dayconfig)
}

/// 将比赛日配置序列化为JSON字符串，排除null字段
pub fn save_day_config(config: &ContestDayConfig) -> Result<String, Box<dyn std::error::Error>> {
    let json_value = serde_json::to_value(config)?;
    let filtered_obj = json_value
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>()
        })
        .ok_or("Failed to convert day config to object")?;
    let json = serde_json::to_string_pretty(&filtered_obj)?;
    Ok(json)
}
