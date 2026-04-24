use crate::prelude::*;
use crate::tuack_lib::config::CONFIG_VERSION;
use crate::tuack_lib::config::migrate::base::Migrater;
use crate::tuack_lib::config::migrate::v3::V3Migrater;
use indexmap::IndexMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestDayConfigFile {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub use_pretest: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub noi_style: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_io: Option<bool>,
}

impl From<ContestDayConfig> for ContestDayConfigFile {
    fn from(config: ContestDayConfig) -> Self {
        ContestDayConfigFile {
            version: config.version,
            folder: config.folder,
            name: config.name,
            subdir: config.subdir,
            title: config.title,
            compile: config.compile,
            start_time: config.start_time,
            end_time: config.end_time,
            use_pretest: config.use_pretest,
            noi_style: config.noi_style,
            file_io: config.file_io,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ContestDayConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    pub compile: HashMap<String, String>,
    pub start_time: [u32; 6],
    pub end_time: [u32; 6],
    pub use_pretest: Option<bool>,
    pub noi_style: Option<bool>,
    pub file_io: Option<bool>,
    pub subconfig: IndexMap<String, ProblemConfig>,
    pub path: PathBuf,
}

/// 加载比赛日配置
pub fn load_day_config(dayconfig_path: &Path) -> Result<ContestDayConfig> {
    // 读取并验证每日配置文件
    let day_content = fs::read_to_string(dayconfig_path)?;
    let mut day_json_value: serde_json::Value = serde_json::from_str(&day_content)?;

    let version = day_json_value
        .get("version")
        .and_then(|v| v.as_u64())
        .context("配置文件缺少版本号")?;

    // 检查版本
    if version < 3 {
        msg_error!(
            "配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。"
        );
        bail!("配置文件版本过低");
    }

    if version > CONFIG_VERSION {
        msg_error!("配置文件版本过高，可能是新版本的配置文件。请检查是否有新版本。");
        bail!("配置文件版本过高");
    }

    if version == 3 {
        info!("正在迁移 V3 天配置文件");
        day_json_value = V3Migrater::migrate_day(day_json_value)?;
    }

    let dayconfig: ContestDayConfigFile = serde_json::from_value(day_json_value)?;

    Ok(ContestDayConfig {
        version: dayconfig.version,
        folder: dayconfig.folder,
        name: dayconfig.name,
        subdir: dayconfig.subdir,
        title: dayconfig.title,
        compile: dayconfig.compile,
        start_time: dayconfig.start_time,
        end_time: dayconfig.end_time,
        use_pretest: dayconfig.use_pretest,
        noi_style: dayconfig.noi_style,
        file_io: dayconfig.file_io,
        subconfig: IndexMap::new(),
        path: dayconfig_path
            .parent()
            .map(|p| p.to_path_buf())
            .context("无法获取配置文件父目录")?,
    })
}

/// 将比赛日配置序列化为 JSON 字符串
pub fn save_day_config(config: &ContestDayConfig) -> Result<String> {
    let dayconfig_file: ContestDayConfigFile = config.clone().into();
    let json = serde_json::to_string_pretty(&dayconfig_file)?;
    Ok(json)
}
