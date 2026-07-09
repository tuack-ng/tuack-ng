use crate::config::msgs::LoadContext;
use crate::prelude::*;

pub const CONFIG_FILE_NAME: &str = "conf.json";

pub const CONFIG_VERSION: u64 = 6;
pub const CONFIG_MIN_VERSION: u64 = 3;

pub mod contest;
pub mod contestday;
pub mod lang;
pub mod migrate;
pub mod msgs;
pub mod problem;

use crate::context::CurrentLocation;

pub use self::contest::*;
pub use self::contestday::*;
pub use self::problem::*;

fn find_contest_config(start_path: &Path) -> Result<PathBuf> {
    let start = dunce::canonicalize(start_path)?;

    for ancestor in start.ancestors() {
        debug!("正在查找配置文件路径：{:?}", ancestor);

        let config_path = ancestor.join(CONFIG_FILE_NAME);
        if config_path.exists() && is_contest_config(&config_path)? {
            return Ok(config_path);
        }
    }

    info!("未找到 contest 配置文件");
    bail!("未找到 contest 配置文件");
}

fn is_contest_config(path: &Path) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let json_value: serde_json::Value = serde_json::from_str(&content)?;

    if let Some(folder) = json_value.get("folder").and_then(|v| v.as_str())
        && folder == "contest"
    {
        Ok(true)
    } else {
        Ok(false)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub config: ContestConfig,
    pub location: CurrentLocation,
}

pub fn load_config(ctx: &mut LoadContext, path: &Path) -> Result<Option<Config>> {
    let config_path = match find_contest_config(path) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    let canonicalize_path = dunce::canonicalize(path)?.to_path_buf();

    ctx.enter();

    // 使用 load_contest_config 加载主配置
    let mut config = load_contest_config(ctx, &config_path)?;

    ctx.set_name(format!("[contest] {}", &config.name));

    let mut location: CurrentLocation = CurrentLocation::None;

    if canonicalize_path.starts_with(config_path.parent().unwrap()) {
        location = CurrentLocation::Root;
    }

    let parent_dir = config_path.parent().context("无法获取配置文件父目录")?;

    // 递归加载子配置
    for day_name in &config.subdir {
        let day_path = parent_dir.join(day_name).join(CONFIG_FILE_NAME);
        ctx.enter();
        let mut dayconfig = load_day_config(ctx, &day_path)?;
        ctx.set_name(format!("[day] {}", &dayconfig.name));

        if canonicalize_path.starts_with(day_path.parent().unwrap()) {
            location = CurrentLocation::Day(day_name.to_string());
        }

        // 递归加载题目配置
        let day_parent_dir = day_path.parent().context("无法获取配置文件父目录")?;
        for problem_name in &dayconfig.subdir {
            let problem_path = day_parent_dir.join(problem_name).join(CONFIG_FILE_NAME);
            ctx.enter();
            let mut problemconfig = load_problem_config(ctx, &problem_path)?;
            ctx.set_name(format!("[problem] {}", &problemconfig.name));
            // TODO：总觉得不对劲
            problemconfig.use_pretest = dayconfig.use_pretest.or(config.use_pretest);
            problemconfig.noi_style = dayconfig.noi_style.or(config.noi_style);
            problemconfig.file_io = if problemconfig.problem_type == ProblemType::Interactive {
                // 交互强制使用 Stdio
                Some(false)
            } else {
                None
            }
            .or(dayconfig.file_io)
            .or(config.file_io);

            if canonicalize_path.starts_with(problem_path.parent().unwrap()) {
                location = CurrentLocation::Problem(day_name.to_string(), problem_name.to_string());
            }

            dayconfig
                .subconfig
                .insert(problem_name.to_string(), problemconfig);

            ctx.ret(); // problem
        }

        config.subconfig.insert(day_name.to_string(), dayconfig);

        ctx.ret(); // day
    }

    ctx.ret(); // contest

    Ok(Some(Config { config, location }))
}

/// 将整个配置序列化并保存到文件系统中
pub fn save_config(config: &ContestConfig, base_path: &Path) -> Result<()> {
    // 检查基础目录是否存在
    if !base_path.exists() {
        bail!("基础目录 {} 不存在", base_path.display());
    }

    // 保存主配置文件
    let main_config_path = base_path.join(CONFIG_FILE_NAME);
    let main_config_json = save_contest_config(config)?;
    fs::write(&main_config_path, main_config_json)?;

    // 保存每个比赛日的配置
    for (day_name, day_config) in config.subconfig.iter() {
        let day_path = base_path.join(day_name);

        // 检查比赛日目录是否存在
        if !day_path.exists() {
            bail!("比赛日目录 {} 不存在", day_path.display());
        }

        let day_config_path = day_path.join(CONFIG_FILE_NAME);
        let day_config_json = save_day_config(day_config)?;
        fs::write(&day_config_path, day_config_json)?;

        // 保存每个题目的配置
        for (problem_name, problem_config) in day_config.subconfig.iter() {
            let problem_path = day_path.join(problem_name);

            // 检查题目目录是否存在
            if !problem_path.exists() {
                bail!("题目目录 {} 不存在", problem_path.display());
            }

            let problem_config_path = problem_path.join(CONFIG_FILE_NAME);
            let problem_config_json = save_problem_config(problem_config)?;
            fs::write(&problem_config_path, problem_config_json)?;
        }
    }

    Ok(())
}
