use crate::config::ExpandedDataItem;
use crate::context::{CurrentLocation, get_context};
use crate::prelude::*;
use crate::utils::compile::{build_compile_cmd, build_run_cmd};
use crate::utils::random::gen_rnd;
use clap::Args;
use clap::ValueEnum;
use indicatif::ProgressBar;
use rand::Rng;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// 正式测试数据
    Data,
    /// 样例数据
    Sample,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Data => write!(f, "data"),
            Target::Sample => write!(f, "sample"),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum DmkCommand {
    /// 生成（未生成的）数据
    Gen,
    /// 重新生成数据（使用相同种子）
    Regen,
    /// 重置种子
    Reset,
}

#[derive(Args, Debug)]
#[command(version, about = "数据生成工具")]
pub struct DmkArgs {
    /// 目标类型
    #[arg(value_enum)]
    pub target: Target,

    /// 操作
    #[arg(value_enum)]
    pub action: DmkCommand,

    /// 操作对象，使用 `,` 和 `-` 分割 (如 1,2-3,4-10)
    #[arg(default_value = "all")]
    object: String,
}

/// 从字符串解析测试点ID集合
pub fn parse_test_object(s: &str, all_ids: &[u32]) -> Result<HashSet<u32>> {
    let s = s.trim().to_lowercase();

    if s == "all" {
        return Ok(all_ids.iter().copied().collect());
    }

    let mut result = HashSet::new();
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        if let Some(pos) = part.find('-') {
            let start_str = &part[..pos];
            let end_str = &part[pos + 1..];

            let start = start_str
                .parse::<u32>()
                .map_err(|_| anyhow!("无效的起始ID: {}", start_str))?;
            let end = end_str
                .parse::<u32>()
                .map_err(|_| anyhow!("无效的结束ID: {}", end_str))?;

            if start > end {
                return Err(anyhow!("起始ID不能大于结束ID: {}", part));
            }

            for id in start..=end {
                if all_ids.contains(&id) {
                    result.insert(id);
                }
            }
        } else {
            let id = part
                .parse::<u32>()
                .map_err(|_| anyhow!("无效的测试点ID: {}", part))?;

            if all_ids.contains(&id) {
                result.insert(id);
            }
        }
    }

    Ok(result)
}

pub fn main(args: DmkArgs) -> Result<()> {
    let config = get_context()
        .config
        .as_ref()
        .context("没有找到有效的工程")?;

    let (current_problem, current_day) =
        if let CurrentLocation::Problem(ref day, ref prog) = config.1 {
            let day_config = config
                .0
                .subconfig
                .get(day)
                .context(format!("无法获取天配置: {}", day))?;

            let problem_config = day_config
                .subconfig
                .get(prog)
                .context(format!("无法获取题目配置: {}/{}", day, prog))?;

            (problem_config, day_config)
        } else {
            bail!("本命令只能在题目目录下执行");
        };

    gen_data(&args, current_problem, current_day)
}

fn gen_data(
    args: &DmkArgs,
    current_problem: &crate::config::ProblemConfig,
    current_day: &crate::config::ContestDayConfig,
) -> Result<(), anyhow::Error> {
    info!("开始生成数据: {}", current_problem.name);
    let target_dir = match args.target {
        Target::Data => current_problem.path.join("data"),
        Target::Sample => current_problem.path.join("sample"),
    };
    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
        info!("创建目标目录: {}", target_dir.display());
    }
    let generator_path = find_generator(&current_problem.path)?;
    let std_path = find_std(current_problem)?;
    info!("找到生成器: {}", generator_path.display());
    info!("找到标程: {}", std_path.display());

    // 并行编译生成器和标程
    let (result1, result2) = rayon::join(
        || compile_generator(&generator_path),
        || compile_std(&std_path, current_problem, current_day),
    );
    result1?;
    result2?;

    let data_items: Vec<Arc<ExpandedDataItem>> = match args.target {
        Target::Data => current_problem.data.to_vec(),
        Target::Sample => current_problem
            .samples
            .iter()
            .map(|item| {
                Arc::new(ExpandedDataItem {
                    id: item.id,
                    score: 0,
                    subtask: 0,
                    input: item.input.get().unwrap().clone(),
                    output: item.output.get().unwrap().clone(),
                    args: item.args.clone(),
                    manual: item.manual.unwrap_or(false),
                })
            })
            .collect(),
    };

    let data_items: Vec<Arc<ExpandedDataItem>> =
        data_items.into_iter().filter(|item| !item.manual).collect();

    let all_ids: Vec<u32> = data_items.iter().map(|data| data.id).collect();
    let target_ids = parse_test_object(&args.object, &all_ids)?;
    let data_items_to_gen: Vec<Arc<ExpandedDataItem>> = data_items
        .into_iter()
        .filter(|item| target_ids.contains(&item.id))
        .collect();

    let seeds = get_or_generate_seed(
        &target_dir,
        matches!(args.action, DmkCommand::Reset),
        &data_items_to_gen,
    )?;

    if data_items_to_gen.is_empty() {
        warn!("没有需要生成的数据");
        return Ok(());
    }

    let pb = get_context()
        .multiprogress
        .add(ProgressBar::new(data_items_to_gen.len() as u64));
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    for data_item in data_items_to_gen {
        pb.set_message(format!("生成数据点 #{}", data_item.id));

        let input_file = data_item.input.clone();
        let output_file = data_item.output.clone();

        let input_path = target_dir.join(&input_file);
        let output_path = target_dir.join(&output_file);

        if !matches!(args.action, DmkCommand::Gen) || !input_path.exists() {
            let mut args_map = current_problem.args.clone();
            args_map.extend(data_item.args.clone());
            generate_input(
                &generator_path,
                &input_path,
                &seeds,
                data_item.id,
                &args_map,
            )?;
        }

        if !matches!(args.action, DmkCommand::Gen) || !output_path.exists() {
            generate_output(
                &std_path,
                &input_path,
                &output_path,
                &current_problem.name,
                current_problem.file_io.unwrap_or(true),
            )?;
        }

        pb.inc(1);
    }

    pb.finish_with_message("数据生成完成！");
    let _ = std::fs::remove_dir_all(std_path.parent().unwrap().join("tmp"));
    save_seed(&target_dir, seeds)?;
    Ok(())
}

/// 查找数据生成器
fn find_generator(problem_path: &std::path::Path) -> Result<std::path::PathBuf> {
    let path = problem_path.join("gen").join("gen.cpp");

    if path.exists() {
        return Ok(path);
    }

    bail!("未找到数据生成器文件")
}

/// 查找标程
fn find_std(problem: &crate::config::ProblemConfig) -> Result<std::path::PathBuf> {
    for (name, case) in &problem.tests {
        if let crate::config::ExpectedScore::Single(str) = &case.expected
            && str.replace(' ', "") == "==100"
            && problem
                .path
                .join(std::path::PathBuf::from(&case.path))
                .exists()
        {
            info!("找到标称 {name}, 位置 {}", case.path);
            return Ok(problem.path.join(std::path::PathBuf::from(&case.path)));
        }
    }

    bail!("未找到标程文件")
}

/// 编译生成器
fn compile_generator(generator_path: &std::path::Path) -> Result<()> {
    info!("编译数据生成器");

    let tmp_dir = generator_path.parent().unwrap();

    let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));
    compile_pb.set_message("编译数据生成器");

    let output_path = tmp_dir.join("gen");

    let status = Command::new("g++")
        .arg("-o")
        .arg(&output_path)
        .arg(generator_path)
        .arg("-O2")
        .arg("-std=c++17")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    compile_pb.finish_and_clear();

    if !status.status.success() {
        error!(
            "数据生成器编译错误: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        bail!("数据生成器编译失败");
    }

    info!("数据生成器编译成功");
    Ok(())
}

/// 编译标程
fn compile_std(
    std_path: &std::path::Path,
    problem: &crate::config::ProblemConfig,
    day: &crate::config::ContestDayConfig,
) -> Result<()> {
    info!("编译标程: {}", std_path.display());

    let tmp_dir = std_path.parent().unwrap().join("tmp");
    create_or_clear_dir(&tmp_dir)?;

    let src_path = tmp_dir.join(std_path.file_name().unwrap());
    std::fs::copy(std_path, &src_path)?;

    let program_name = problem.name.clone();

    let compile_cmd = build_compile_cmd(&src_path, &tmp_dir, &program_name, &day.compile)?;

    let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));
    compile_pb.set_message("编译标程");

    if let Some(mut cmd) = compile_cmd {
        let status = cmd
            .current_dir(&tmp_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;

        compile_pb.finish_and_clear();

        if !status.success() {
            if let Ok(output) = cmd.output() {
                error!("标程编译错误: {}", String::from_utf8_lossy(&output.stderr));
            }
            bail!("标程编译失败");
        }

        info!("标程编译成功");
    } else {
        // 对于无需编译的语言，复制源文件
        let target_path = tmp_dir
            .join(&program_name)
            .with_extension(std_path.extension().unwrap_or_default());
        std::fs::copy(&src_path, &target_path)?;
        compile_pb.finish_and_clear();
        info!("标程准备完成");
    }

    Ok(())
}

/// 创建或清空目录
fn create_or_clear_dir(path: &std::path::Path) -> Result<(), std::io::Error> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    std::fs::create_dir_all(path)
}

/// 获取或生成种子
fn get_or_generate_seed(
    target_dir: &std::path::Path,
    force: bool,
    data: &[Arc<ExpandedDataItem>],
) -> Result<BTreeMap<u32, u64>> {
    let mut rng = gen_rnd()?;
    let mut seeds: BTreeMap<u32, u64> = BTreeMap::new();

    let seed_file = target_dir.join(".seed");

    if !force && seed_file.exists() {
        let seed_str = std::fs::read_to_string(&seed_file)?;
        seeds = serde_json::from_str(&seed_str).unwrap_or_else(|e| {
            warn!(".seed 文件无效, 重新生成: {}", e);
            BTreeMap::new()
        });
    }

    for item in data {
        let id = item.id;
        seeds.entry(id).or_insert_with(|| rng.random::<u64>());
    }

    Ok(seeds)
}

/// 保存种子
fn save_seed(target_dir: &std::path::Path, seeds: BTreeMap<u32, u64>) -> Result<()> {
    let seed_file = target_dir.join(".seed");
    std::fs::write(seed_file, serde_json::to_string_pretty(&seeds)?)?;
    Ok(())
}

/// 生成输入文件
fn generate_input(
    generator_path: &std::path::Path,
    input_path: &std::path::Path,
    seeds: &BTreeMap<u32, u64>,
    test_id: u32,
    args: &HashMap<String, i64>,
) -> Result<()> {
    let tmp_dir = generator_path.parent().unwrap();
    let generator_exe = tmp_dir.join("gen");

    if !generator_exe.exists() {
        bail!("生成器未编译");
    }

    // 构建参数列表
    let mut cmd_args = vec![test_id.to_string()];

    // 添加自定义参数
    for (key, value) in args {
        cmd_args.push(format!("-{}={}", key, value));
    }

    cmd_args.push("-seed".to_string());
    cmd_args.push(seeds.get(&test_id).unwrap().to_string());

    // 运行生成器
    let output = Command::new(&generator_exe)
        .args(&cmd_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("生成器运行失败 (测试点 {}): {}", test_id, stderr);
        bail!("生成器运行失败");
    }

    // 写入输入文件
    std::fs::write(input_path, &output.stdout)?;

    debug!("生成输入文件: {}", input_path.display(),);
    Ok(())
}

/// 使用标程生成输出文件
fn generate_output(
    std_path: &std::path::Path,
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    problem_name: &str,
    file_io: bool,
) -> Result<()> {
    let tmp_dir = std_path.parent().unwrap().join("tmp");

    // 准备工作目录
    let work_dir = &tmp_dir;

    // 复制输入文件到工作目录
    let work_input_path = if file_io {
        work_dir.join(format!("{}.in", problem_name))
    } else {
        work_dir.join(format!("{}.stdin", problem_name))
    };

    fs::copy(input_path, &work_input_path)?;
    debug!("复制输入文件到工作目录: {}", work_input_path.display());

    // 准备输出路径
    let work_output_path = if file_io {
        work_dir.join(format!("{}.out", problem_name))
    } else {
        work_dir.join(format!("{}.stdout", problem_name))
    };

    let mut cmd = if let Some(cmd) = build_run_cmd(std_path, work_dir, problem_name)? {
        cmd
    } else {
        let exe_extension = std::env::consts::EXE_EXTENSION;
        let executable_path = work_dir.join(problem_name).with_extension(exe_extension);

        if !executable_path.exists() {
            error!("找不到可执行文件: {}", executable_path.display());
            bail!("找不到标程可执行文件");
        }

        debug!("使用可执行文件: {}", executable_path.display());
        std::process::Command::new(executable_path)
    };

    // 设置IO重定向
    let child = if file_io {
        cmd.current_dir(work_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
    } else {
        let input_file = std::fs::File::open(&work_input_path)?;
        let output_file = std::fs::File::create(&work_output_path)?;

        cmd.current_dir(work_dir)
            .stdin(Stdio::from(input_file))
            .stdout(Stdio::from(output_file))
            .stderr(Stdio::piped())
    };

    // 运行标程
    debug!("运行标程命令");
    let output = child.output()?;

    if !output.status.success() {
        error!("标程运行失败，退出码: {}", output.status);
        if !output.stderr.is_empty() {
            error!("错误输出: {}", String::from_utf8_lossy(&output.stderr));
        }
        bail!("标程运行失败");
    }

    // 检查输出文件是否生成
    if !work_output_path.exists() {
        error!("标程未生成输出文件: {}", work_output_path.display());
        bail!("标程未生成输出文件");
    }

    // 复制输出文件到目标位置
    std::fs::copy(&work_output_path, output_path)?;

    debug!("成功生成输出文件: {}", output_path.display());

    info!("标程成功生成输出");
    Ok(())
}
