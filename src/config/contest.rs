use crate::{
    config::{CONFIG_MIN_VERSION, CONFIG_VERSION, migrate::base::MIGRATERS, msgs::LoadContext},
    prelude::*,
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

impl From<ContestConfig> for ContestConfigFile {
    fn from(val: ContestConfig) -> Self {
        ContestConfigFile {
            version: val.version,
            folder: val.folder,
            name: val.name,
            subdir: val.subdir,
            title: val.title,
            short_title: val.short_title,
            use_pretest: val.use_pretest,
            noi_style: val.noi_style,
            file_io: val.file_io,
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
pub fn load_contest_config(ctx: &mut LoadContext, config_path: &Path) -> Result<ContestConfig> {
    // 读取并验证主配置文件
    let main_content = fs::read_to_string(config_path)?;
    let mut main_json_value: serde_json::Value = serde_json::from_str(&main_content)?;

    // 检查版本
    let mut version = main_json_value
        .get("version")
        .and_then(|v| v.as_u64())
        .context("配置文件缺少版本号")?;

    if version < CONFIG_MIN_VERSION {
        bail!("配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。");
    }

    if version > CONFIG_VERSION {
        bail!("配置文件版本过高，可能是新版本的配置文件。请检查是否有新版本。");
    }

    while version < CONFIG_VERSION {
        match MIGRATERS.get(&(version as i32)) {
            Some(migrater) => {
                if migrater.metadata().force && !ctx.force_migrate() {
                    bail!(
                        "配置文件已经过时且无法自动迁移。你需要使用 `tuack-ng conf migrate` 手动迁移。"
                    )
                } else {
                    let from_ver = version as i32;
                    main_json_value = migrater.migrate_contest(main_json_value, config_path.parent().unwrap())?;
                    version = main_json_value
                        .get("version")
                        .and_then(|v| v.as_u64())
                        .context("配置文件缺少版本号")?;
                    ctx.migrated = true;
                    if let Some(notice) = migrater.metadata().notice {
                        ctx.migrated_notices.entry(from_ver).or_insert(notice);
                    }
                }
            }
            None => bail!("不存在配置文件版本 {} 的迁移", version),
        }
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
