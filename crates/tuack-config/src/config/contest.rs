use crate::{
    config::{CONFIG_MIN_VERSION, CONFIG_VERSION, migrate::base::MIGRATERS, msgs::LoadContext},
    prelude::*,
};

use crate::config::contestday::ContestDayConfig;

#[derive(Debug, Clone, DeserializeMany, SerializeMany)]
#[serde_many(file = "FileView", full = "FullView")]
#[serde(file(rename_all = "kebab-case"), full(rename_all = "kebab-case"))]
pub struct ContestConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    #[serde(file(rename = "short title"), full(rename = "short title"))]
    pub short_title: String,
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub use_pretest: Option<bool>,
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub noi_style: Option<bool>,
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub file_io: Option<bool>,

    // 运行时信息
    #[serde(file(skip))]
    pub subconfig: IndexMapMany<String, ContestDayConfig>,
    #[serde(file(skip))]
    pub path: PathBuf,
}

impl ContestConfig {
    pub fn load(ctx: &mut LoadContext, config_path: &Path) -> Result<Self> {
        // 读取并验证主配置文件
        let main_content = fs::read_to_string(config_path)?;
        let mut main_json_value: serde_json::Value = serde_json::from_str(&main_content)?;

        // 检查版本
        let mut version = main_json_value
            .get("version")
            .and_then(|v| v.as_u64())
            .context("配置文件缺少版本号")?;

        if version < CONFIG_MIN_VERSION {
            bail!(
                "配置文件版本过低，可能是 Tuack 的配置文件。请迁移到 Tuack-NG 配置文件格式再使用。"
            );
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
                        main_json_value = migrater
                            .migrate_contest(main_json_value, config_path.parent().unwrap())?;
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
        let mut config: ContestConfig =
            serde_json::from_value::<AsSerde<ContestConfig, FileView>>(main_json_value)?
                .into_inner();

        config.path = config_path.parent().unwrap().to_path_buf();

        Ok(config)
    }

    pub fn save(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&AsSerde::<
            ContestConfig,
            FileView,
        >::new(self.clone()))?)
    }
}
