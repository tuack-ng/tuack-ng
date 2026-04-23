use crate::prelude::*;
use indexmap::IndexMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestConfigFile {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    #[serde(rename = "short title")]
    pub short_title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_pretest: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noi_style: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_io: Option<bool>,
}

impl Into<ContestConfigFile> for ContestConfig {
    fn into(self) -> ContestConfigFile {
        ContestConfigFile {
            version: self.version,
            folder: self.folder,
            name: self.name,
            subdir: self.subdir,
            title: self.title,
            short_title: self.short_title,
            use_pretest: self.use_pretest,
            noi_style: self.noi_style,
            file_io: self.file_io,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    pub short_title: String,
    pub use_pretest: Option<bool>,
    pub noi_style: Option<bool>,
    pub file_io: Option<bool>,
    pub subconfig: IndexMap<String, ContestDayConfig>,
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
        msg_error!(
            "配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。"
        );
        bail!("配置文件版本过低");
    }

    // 反序列化主配置
    let config: ContestConfigFile = serde_json::from_str(&main_content)?;

    Ok(ContestConfig {
        version: config.version,
        folder: config.folder,
        name: config.name,
        subdir: config.subdir,
        title: config.title,
        short_title: config.short_title,
        use_pretest: config.use_pretest,
        noi_style: config.noi_style,
        file_io: config.file_io,

        subconfig: IndexMap::new(),
        path: config_path.parent().unwrap().to_path_buf(),
    })
}

/// 将比赛配置序列化为 JSON 字符串
pub fn save_contest_config(config: &ContestConfig) -> Result<String> {
    let config_file: ContestConfigFile = config.clone().into();
    let json = serde_json::to_string_pretty(&config_file)?;
    Ok(json)
}
