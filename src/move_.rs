use crate::config::find_contest_config;
use crate::config::CONFIG_FILE_NAME;
use crate::prelude::*;
use clap::Args;

#[derive(Args, Debug, Clone)]
#[command(version)]
pub struct MoveArgs {
    /// 目标位置（从 1 开始）
    #[arg(short = 'x', long = "pos", default_value = "1")]
    pub position: usize,
    /// 题目名称（目录名）
    pub problem: String,
    /// 目标比赛日目录名
    pub day: String,
}

pub fn main(args: MoveArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;

    let config_path = find_contest_config(&current_dir)
        .context("未找到 contest 配置文件，请在 contest 目录或其子目录下运行")?;
    let contest_root = config_path
        .parent()
        .context("无法获取 contest 根目录")?
        .to_path_buf();

    let day_list: Vec<String> = {
        let content = fs::read_to_string(&config_path)?;
        let contest_json: serde_json::Value = serde_json::from_str(&content)?;
        contest_json["subdir"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };

    let source_day = day_list
        .iter()
        .find(|day_name| {
            contest_root
                .join(day_name)
                .join(&args.problem)
                .join(CONFIG_FILE_NAME)
                .exists()
        })
        .cloned()
        .context(format!("找不到题目 '{}'", args.problem))?;

    let target_day_path = contest_root.join(&args.day);
    if !target_day_path.join(CONFIG_FILE_NAME).exists() {
        bail!("目标比赛日 '{}' 不存在", args.day);
    }

    let target_problem_path = target_day_path.join(&args.problem);
    if target_problem_path.exists() && source_day != args.day {
        bail!(
            "目标比赛日 '{}' 已存在同名题目 '{}'",
            args.day,
            args.problem
        );
    }

    let source_day_path = contest_root.join(&source_day);
    let source_problem_path = source_day_path.join(&args.problem);
    let source_config_path = source_day_path.join(CONFIG_FILE_NAME);

    let source_content = fs::read_to_string(&source_config_path)?;
    let mut source_json: serde_json::Value = serde_json::from_str(&source_content)?;
    let source_subdir = source_json["subdir"]
        .as_array_mut()
        .context("源比赛日配置文件缺少 subdir 字段")?;
    source_subdir.retain(|v| v.as_str() != Some(&args.problem));

    let target_config_path = target_day_path.join(CONFIG_FILE_NAME);
    let target_content = fs::read_to_string(&target_config_path)?;
    let mut target_json: serde_json::Value = serde_json::from_str(&target_content)?;
    let target_subdir = target_json["subdir"]
        .as_array_mut()
        .context("目标比赛日配置文件缺少 subdir 字段")?;

    let pos = if args.position == 0 {
        0
    } else {
        args.position - 1
    };
    let pos = pos.min(target_subdir.len());
    target_subdir.insert(pos, serde_json::Value::String(args.problem.clone()));

    if source_day != args.day {
        fs::rename(&source_problem_path, &target_problem_path)
            .with_context(|| {
                format!(
                    "移动题目目录失败：{} -> {}",
                    source_problem_path.display(),
                    target_problem_path.display()
                )
            })?;
    }

    let updated_source = serde_json::to_string_pretty(&source_json)?;
    fs::write(&source_config_path, updated_source)?;

    let updated_target = serde_json::to_string_pretty(&target_json)?;
    fs::write(&target_config_path, updated_target)?;

    if source_day == args.day {
        msg_info!(
            "已将题目 '{}' 在比赛日 '{}' 中重新排序为第 {} 题",
            args.problem,
            args.day,
            args.position
        );
    } else {
        msg_info!(
            "已将题目 '{}' 从 '{}' 移动到 '{}' 的第 {} 题",
            args.problem,
            source_day,
            args.day,
            args.position
        );
    }

    Ok(())
}
