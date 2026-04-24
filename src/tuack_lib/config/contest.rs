use crate::{
    prelude::*,
    tuack_lib::config::{
        CONFIG_VERSION,
        migrate::{base::Migrater, v3::V3Migrater},
    },
};
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
    let mut main_json_value: serde_json::Value = serde_json::from_str(&main_content)?;

    // 检查版本

    let version = main_json_value
        .get("version")
        .and_then(|v| v.as_u64())
        .context("配置文件缺少版本号")?;

    let mut migrated = false;

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
        info!("正在迁移 V3 比赛配置文件");
        main_json_value = V3Migrater::migrate_contest(main_json_value)?;
        migrated = true;
    }

    if migrated {
        // TODO: 不应该在这里提示
        msg_warn!("配置文件版本已经过时。使用 `tuack-ng conf migrate` 进行迁移。");
    }

    // 反序列化主配置
    let config: ContestConfigFile = serde_json::from_value(main_json_value)?;

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
