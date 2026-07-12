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
    /// 目标比赛日目录名（支持 ../ ./ 等相对路径，可跨 contest）
    pub day: String,
}

pub fn main(args: MoveArgs) -> Result<()> {
    let current_dir = std::env::current_dir()?;

    let source_config_path = find_contest_config(&current_dir)
        .context("未找到 contest 配置文件，请在 contest 目录或其子目录下运行")?;
    let source_contest_root = source_config_path
        .parent()
        .context("无法获取 contest 根目录")?
        .to_path_buf();

    let source_day_list: Vec<String> = {
        let content = fs::read_to_string(&source_config_path)?;
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

    let source_day = source_day_list
        .iter()
        .find(|day_name| {
            source_contest_root
                .join(day_name)
                .join(&args.problem)
                .join(CONFIG_FILE_NAME)
                .exists()
        })
        .cloned()
        .context(format!("找不到题目 '{}'", args.problem))?;

    let (target_contest_root, target_day_name) =
        resolve_target(&args.day, &current_dir, &source_contest_root)?;

    let target_day_path = target_contest_root.join(&target_day_name);
    let target_day_config_path = target_day_path.join(CONFIG_FILE_NAME);
    if !target_day_config_path.exists() {
        bail!("目标比赛日 '{}' 不存在", target_day_name);
    }

    let target_problem_path = target_day_path.join(&args.problem);
    let same_contest = source_contest_root == target_contest_root;

    if !same_contest || source_day != target_day_name {
        if target_problem_path.exists() {
            bail!(
                "目标比赛日 '{}' 已存在同名题目 '{}'",
                target_day_name,
                args.problem
            );
        }
    }

    let source_day_path = source_contest_root.join(&source_day);
    let source_problem_path = source_day_path.join(&args.problem);
    let source_day_config_path = source_day_path.join(CONFIG_FILE_NAME);

    let source_content = fs::read_to_string(&source_day_config_path)?;
    let mut source_json: serde_json::Value = serde_json::from_str(&source_content)?;
    let source_subdir = source_json["subdir"]
        .as_array_mut()
        .context("源比赛日配置文件缺少 subdir 字段")?;
    source_subdir.retain(|v| v.as_str() != Some(&args.problem));

    let target_content = fs::read_to_string(&target_day_config_path)?;
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

    if source_day != target_day_name || !same_contest {
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
    fs::write(&source_day_config_path, updated_source)?;

    let updated_target = serde_json::to_string_pretty(&target_json)?;
    fs::write(&target_day_config_path, updated_target)?;

    if same_contest && source_day == target_day_name {
        msg_info!(
            "已将题目 '{}' 在比赛日 '{}' 中重新排序为第 {} 题",
            args.problem,
            target_day_name,
            args.position
        );
    } else if same_contest {
        msg_info!(
            "已将题目 '{}' 从 '{}' 移动到 '{}' 的第 {} 题",
            args.problem,
            source_day,
            target_day_name,
            args.position
        );
    } else {
        msg_info!(
            "已将题目 '{}' 从 contest '{}' 的 '{}' 移动到 contest '{}' 的 '{}' 的第 {} 题",
            args.problem,
            source_contest_root.file_name().unwrap().to_string_lossy(),
            source_day,
            target_contest_root.file_name().unwrap().to_string_lossy(),
            target_day_name,
            args.position
        );
    }

    Ok(())
}

/// 解析目标路径，返回所在 contest 根目录和比赛日名称。
///
/// - 简单名称（不含 `/`、`\`、`.`、`..`）：先尝试在当前 contest 下查找，未命中则
///   尝试作为路径解析。
/// - 否则相对 `current_dir` 解析绝对路径，通过 `find_contest_config` 定位所属 contest。
fn resolve_target(
    raw: &str,
    current_dir: &Path,
    source_contest_root: &Path,
) -> Result<(PathBuf, String)> {
    // 简单名称：优先在当前 contest 下匹配
    if !raw.contains(['/', '\\']) && raw != "." && raw != ".." {
        let day_path = source_contest_root.join(raw);
        if day_path.join(CONFIG_FILE_NAME).exists() {
            return Ok((source_contest_root.to_path_buf(), raw.to_string()));
        }
        // 当前 contest 下没有 → 尝试作为路径解析
    }

    let p = PathBuf::from(raw);
    let abs = if p.is_absolute() {
        p
    } else {
        current_dir.join(&p)
    };
    let canon = dunce::canonicalize(&abs)
        .with_context(|| format!("无法解析路径 '{}'", raw))?;

    if canon.starts_with(source_contest_root) {
        match canon.strip_prefix(source_contest_root) {
            Ok(rel) if !rel.as_os_str().is_empty() => {
                let day_name = rel
                    .components()
                    .next()
                    .context("路径指向 contest 根目录，不是比赛日目录")?
                    .as_os_str()
                    .to_string_lossy()
                    .to_string();
                return Ok((source_contest_root.to_path_buf(), day_name));
            }
            _ => {}
        }
    }

    // 可能跨 contest：从目标路径向上找 contest 配置
    let target_config = find_contest_config(&canon)
        .with_context(|| format!("路径 '{}' 不在任何 contest 根目录下", canon.display()))?;
    let target_contest_root = target_config
        .parent()
        .context("无法获取目标 contest 根目录")?
        .to_path_buf();

    if !canon.starts_with(&target_contest_root) {
        bail!(
            "路径 '{}' 不在目标 contest 根目录 '{}' 下",
            canon.display(),
            target_contest_root.display()
        );
    }

    let rel = canon
        .strip_prefix(&target_contest_root)
        .context("无法从路径中提取比赛日名称")?;
    let day_name = rel
        .components()
        .next()
        .context("路径指向 contest 根目录，不是比赛日目录")?
        .as_os_str()
        .to_string_lossy()
        .to_string();

    Ok((target_contest_root, day_name))
}
