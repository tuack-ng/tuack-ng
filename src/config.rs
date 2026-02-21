use crate::prelude::*;

pub const CONFIG_FILE_NAME: &str = "conf.json";

pub mod contest;
pub mod contestday;
pub mod data;
pub mod lang;
pub mod models;
pub mod problem;

use crate::context::CurrentLocation;

pub use self::contest::*;
pub use self::contestday::*;
pub use self::data::*;
pub use self::models::*;
pub use self::problem::*;

fn find_contest_config(start_path: &Path) -> Result<PathBuf> {
    let start = dunce::canonicalize(start_path)?;

    for ancestor in start.ancestors() {
        debug!("Checking: {:?}", ancestor);

        let config_path = ancestor.join(CONFIG_FILE_NAME);
        if config_path.exists() && is_contest_config(&config_path)? {
            return Ok(config_path);
        }
    }

    info!("未找到contest配置文件");
    bail!("未找到contest配置文件");
}

fn is_contest_config(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let json_value: serde_json::Value = serde_json::from_str(&content)?;

    // 通过字段判断是否是contest配置
    if let Some(version) = json_value.get("version").and_then(|v| v.as_u64()) {
        if version >= 3 {
            if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str())
                && folder == "contest"
            {
                return Ok(true);
            }
        } else {
            error!(
                "配置文件版本过低，可能是 tuack 的配置文件。请迁移到 tuack-ng 配置文件格式再使用。"
            );
            bail!("配置文件版本过低");
        }
    }

    Ok(false)
}

pub fn load_config(path: &Path) -> Result<Option<(ContestConfig, CurrentLocation)>> {
    let config_path = match find_contest_config(path) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    let canonicalize_path = dunce::canonicalize(path)?.to_path_buf();

    // 使用 load_contest_config 加载主配置
    let mut config = load_contest_config(&config_path)?;

    let mut location: CurrentLocation = CurrentLocation::None;

    if canonicalize_path.starts_with(config_path.parent().unwrap()) {
        location = CurrentLocation::Root;
    }

    let parent_dir = config_path
        .parent()
        .map(|p| p.to_path_buf())
        .context("无法获取配置文件父目录")?;

    // 递归加载子配置
    for dayconfig_name in &config.subdir {
        let dayconfig_path = parent_dir.join(dayconfig_name).join(CONFIG_FILE_NAME);
        let mut dayconfig = load_day_config(&dayconfig_path)?;

        if canonicalize_path.starts_with(dayconfig_path.parent().unwrap()) {
            location = CurrentLocation::Day(dayconfig_name.to_string());
        }

        // 递归加载题目配置
        let day_parent_dir = dayconfig_path
            .parent()
            .map(|p| p.to_path_buf())
            .context("无法获取配置文件父目录")?;
        for problemconfig_name in &dayconfig.subdir {
            let problemconfig_path = day_parent_dir
                .join(problemconfig_name)
                .join(CONFIG_FILE_NAME);
            let mut problemconfig = load_problem_config(&problemconfig_path)?;

            problemconfig.use_pretest = dayconfig.use_pretest.or(config.use_pretest);
            problemconfig.noi_style = dayconfig.noi_style.or(config.noi_style);
            problemconfig.file_io = dayconfig.file_io.or(config.file_io);

            if canonicalize_path.starts_with(problemconfig_path.parent().unwrap()) {
                location = CurrentLocation::Problem(
                    dayconfig_name.to_string(),
                    problemconfig_name.to_string(),
                );
            }

            dayconfig
                .subconfig
                .insert(problemconfig_name.to_string(), problemconfig);
        }

        config
            .subconfig
            .insert(dayconfig_name.to_string(), dayconfig);
    }

    Ok(Some((config, location)))
}

#[allow(unused)]
/// 将整个配置序列化并保存到文件系统中
pub fn save_config(config: &ContestConfig, base_path: &Path) -> Result<()> {
    // 检查基础目录是否存在
    if !base_path.exists() {
        bail!("基础目录 {} 不存在", base_path.display());
    }

    // 保存主配置文件（排除null字段）
    let main_config_path = base_path.join(CONFIG_FILE_NAME);
    let main_config_json = save_contest_config(config)?;
    fs::write(&main_config_path, main_config_json)?;

    // 保存每个比赛日的配置
    for (day_index, (day_name, day_config)) in config.subconfig.iter().enumerate() {
        if config.subdir.len() <= day_index {
            bail!("子目录名称不足，无法保存第{}个比赛日配置", day_index);
        }

        let day_name = &config.subdir[day_index];
        let day_path = base_path.join(day_name);

        // 检查比赛日目录是否存在
        if !day_path.exists() {
            bail!("比赛日目录 {} 不存在", day_path.display());
        }

        let day_config_path = day_path.join(CONFIG_FILE_NAME);
        let day_config_json = save_day_config(day_config)?;
        fs::write(&day_config_path, day_config_json)?;

        // 保存每个题目的配置
        for (problem_index, problem_config) in day_config.subconfig.iter().enumerate() {
            if day_config.subdir.len() <= problem_index {
                bail!("子目录名称不足，无法保存第{}个题目配置", problem_index);
            }

            let problem_name = &day_config.subdir[problem_index];
            let problem_path = day_path.join(problem_name);

            // 检查题目目录是否存在
            if !problem_path.exists() {
                bail!("题目目录 {} 不存在", problem_path.display());
            }

            let problem_config_path = problem_path.join(CONFIG_FILE_NAME);
            let problem_config_json = save_problem_config(problem_config.1)?;
            fs::write(&problem_config_path, problem_config_json)?;
        }
    }

    Ok(())
}
