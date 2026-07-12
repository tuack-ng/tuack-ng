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
    /// 目标比赛日目录名（支持 ../ ./ 等相对路径，可跨 contest）
    pub day: String,
}

pub fn main(args: MoveArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;

    let source_day_path = find_day_dir(&current_dir)
        .context("未找到比赛日配置文件，请在比赛日目录或其子目录下运行")?;
    let contest_root = source_day_path
        .parent()
        .context("无法获取比赛日父目录")?;
    let source_day_name = source_day_path
        .file_name()
        .context("无效的比赛日路径")?
        .to_string_lossy()
        .to_string();

    let source_problem_path = source_day_path.join(&args.problem);
    let source_problem_config = source_problem_path.join(CONFIG_FILE_NAME);
    if !source_problem_config.exists() {
        bail!("在比赛日 '{}' 中找不到题目 '{}'", source_day_name, args.problem);
    }

    let target_day_path = resolve_day_dir(&args.day, &current_dir, contest_root)
        .context(format!("无法解析目标比赛日 '{}'", args.day))?;
    let target_day_name = target_day_path
        .file_name()
        .context("无效的目标比赛日路径")?
        .to_string_lossy()
        .to_string();

    let target_problem_path = target_day_path.join(&args.problem);
    if source_day_path != target_day_path && target_problem_path.exists() {
        bail!("目标比赛日 '{}' 已存在同名题目 '{}'", target_day_name, args.problem);
    }

    let source_config_path = source_day_path.join(CONFIG_FILE_NAME);
    let source_content = fs::read_to_string(&source_config_path)?;
    let mut source_json: serde_json::Value = serde_json::from_str(&source_content)?;
    let source_subdir = source_json["subdir"]
        .as_array_mut()
        .context("比赛日配置文件缺少 subdir 字段")?;
    source_subdir.retain(|v| v.as_str() != Some(&args.problem));

    let target_config_path = target_day_path.join(CONFIG_FILE_NAME);
    let target_content = fs::read_to_string(&target_config_path)?;
    let mut target_json: serde_json::Value = serde_json::from_str(&target_content)?;
    let target_subdir = target_json["subdir"]
        .as_array_mut()
        .context("比赛日配置文件缺少 subdir 字段")?;

    let pos = if args.position == 0 {
        0
    } else {
        args.position - 1
    };
    let pos = pos.min(target_subdir.len());
    target_subdir.insert(pos, serde_json::Value::String(args.problem.clone()));

    if source_day_path != target_day_path {
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

    if source_day_path == target_day_path {
        msg_info!(
            "已将题目 '{}' 在比赛日 '{}' 中重新排序为第 {} 题",
            args.problem,
            target_day_name,
            args.position
        );
    } else {
        msg_info!(
            "已将题目 '{}' 从 '{}' 移动到 '{}' 的第 {} 题",
            args.problem,
            source_day_name,
            target_day_name,
            args.position
        );
    }

    Ok(())
}

/// 从 `start` 向上查找包含 `conf.json` 且 `folder` 为 `"day"` 的目录。
fn find_day_dir(start: &Path) -> Result<PathBuf> {
    for ancestor in start.ancestors() {
        let conf = ancestor.join(CONFIG_FILE_NAME);
        if conf.exists() {
            let content = fs::read_to_string(&conf)?;
            let json: serde_json::Value = serde_json::from_str(&content)?;
            if json.get("folder").and_then(|f| f.as_str()) == Some("day") {
                return Ok(ancestor.to_path_buf());
            }
        }
    }
    bail!("未找到比赛日配置文件，请确认在比赛日目录下运行");
}

/// 解析用户输入的目标路径，返回对应的比赛日目录。
///
/// - 简单名称（不含路径分隔符，也不是 `.`/`..`）：
///   优先尝试作为 contest_root 下的兄弟比赛日，否则作为路径解析。
/// - 否则：相对 `current_dir` 解析为绝对路径，再向上查找 `folder == "day"` 的目录。
fn resolve_day_dir(raw: &str, current_dir: &Path, contest_root: &Path) -> Result<PathBuf> {
    if !raw.contains(['/', '\\']) && raw != "." && raw != ".." {
        let sibling = contest_root.join(raw);
        if sibling.join(CONFIG_FILE_NAME).exists() {
            return Ok(dunce::canonicalize(&sibling)?);
        }
    }

    let p = PathBuf::from(raw);
    let abs = if p.is_absolute() {
        p
    } else {
        current_dir.join(&p)
    };
    let canon = dunce::canonicalize(&abs)?;

    let mut path = canon;
    loop {
        let conf = path.join(CONFIG_FILE_NAME);
        if conf.exists() {
            if let Ok(content) = fs::read_to_string(&conf) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if json.get("folder").and_then(|f| f.as_str()) == Some("day") {
                        return Ok(path);
                    }
                }
            }
        }
        if !path.pop() {
            break;
        }
    }

    bail!("未能在目标路径下找到比赛日目录");
}
