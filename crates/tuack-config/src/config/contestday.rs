use crate::config::migrate::base::MIGRATERS;
use crate::config::msgs::LoadContext;
use crate::config::problem::ProblemConfig;
use crate::config::{CONFIG_MIN_VERSION, CONFIG_VERSION};
use crate::prelude::*;
use indexmap::IndexMap;

#[derive(Debug, Clone, DeserializeMany, SerializeMany)]
#[serde_many(file = "FileView", full = "FullView")]
#[serde(file(rename_all = "kebab-case"), full(rename_all = "kebab-case"))]
pub struct ContestDayConfig {
    pub version: u32,
    pub folder: String,
    pub name: String,
    pub subdir: Vec<String>,
    pub title: String,
    pub compile: IndexMap<String, String>,
    #[serde(file(rename = "start time"), full(rename = "start time"))]
    pub start_time: Option<[u32; 6]>,
    #[serde(file(rename = "end time"), full(rename = "end time"))]
    pub end_time: Option<[u32; 6]>,
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub use_pretest: Option<bool>,
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub noi_style: Option<bool>,
    #[serde(file(default, skip_serializing_if = "Option::is_none"))]
    pub file_io: Option<bool>,

    // 运行时信息
    #[serde(file(skip))]
    pub subconfig: IndexMapMany<String, ProblemConfig>,
    #[serde(file(skip))]
    pub path: PathBuf,
}

impl ContestDayConfig {
    pub fn load(ctx: &mut LoadContext, config_path: &Path) -> Result<Self> {
        // 读取并验证每日配置文件
        let content = fs::read_to_string(config_path)?;
        let mut json: serde_json::Value = serde_json::from_str(&content)?;

        let mut version = json
            .get("version")
            .and_then(|v| v.as_u64())
            .context("配置文件缺少版本号")?;

        // 检查版本
        if version < CONFIG_MIN_VERSION {
            bail!(
                "配置文件版本过低，可能是 Tuack 的配置文件。请迁移到 Tuack-NG 配置文件格式再使用。"
            );
        }

        if version > CONFIG_VERSION {
            bail!("配置文件版本过高，可能是新版本的配置文件。请检查是否有新版本。");
        }

        let folder = json
            .get("folder")
            .and_then(|v| v.as_str())
            .context("配置文件缺少 `folder` 字段")?;

        if folder != "day" {
            bail!("配置文件层级错误。预期 `day`，读到 `{}`", folder);
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
                        json = migrater.migrate_day(json, config_path.parent().unwrap())?;
                        version = json
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
        let mut config: ContestDayConfig =
            serde_json::from_value::<AsSerde<ContestDayConfig, FileView>>(json)?.into_inner();

        config.path = config_path
            .parent()
            .context("无法获取配置文件父目录")?
            .into();

        Ok(config)
    }

    pub fn save(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(&AsSerde::<
            ContestDayConfig,
            FileView,
        >::new(self.clone()))?)
    }
}
