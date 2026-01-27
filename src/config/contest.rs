use crate::prelude::*;
use indexmap::IndexMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    #[serde(rename = "short title")]
    pub short_title: String,
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
    // pub subconfig: Vec<ContestDayConfig>,
    pub subconfig: IndexMap<String, ContestDayConfig>,
    #[serde(skip)]
    pub path: PathBuf,
}

/// 加载比赛配置
pub fn load_contest_config(config_path: &Path) -> Result<ContestConfig> {
    // 读取并验证主配置文件
    let main_content = fs::read_to_string(config_path)?;
    let main_json_value: serde_json::Value = serde_json::from_str(&main_content)?;

    // 检查版本
    if let Some(version) = main_json_value.get("version").and_then(|v| v.as_u64())
        && version < 3
    {
        error!("配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。");
        bail!("配置文件版本过低");
    }

    // 反序列化主配置
    let mut config: ContestConfig = serde_json::from_str(&main_content)?;

    config.path = config_path.parent().unwrap().to_path_buf();

    config.subconfig = IndexMap::new();

    Ok(config)
}

/// 将比赛配置序列化为JSON字符串，排除null字段
pub fn save_contest_config(config: &ContestConfig) -> Result<String> {
    let json_value = serde_json::to_value(config)?;
    let filtered_obj = json_value
        .as_object()
        .map(|obj| {
            obj.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<serde_json::Map<_, _>>()
        })
        .context("Failed to convert contest config to object")?;
    let json = serde_json::to_string_pretty(&filtered_obj)?;
    Ok(json)
}
