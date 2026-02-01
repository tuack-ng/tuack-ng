use crate::config::ExpandedDataItem;
use crate::context::{CurrentLocation, get_context};
use crate::prelude::*;
use crate::utils::compile::build_compile_cmd;
use crate::utils::random::gen_rnd;
use clap::Args;
use clap::ValueEnum;
use indicatif::ProgressBar;
use rand::Rng;
use std::collections::HashSet;
use std::fmt;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// 正式测试数据
    Data,
    /// 样例数据
    Sample,
    // / 预测试数据
    // Pretest, // 还没写
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Data => write!(f, "data"),
            Target::Sample => write!(f, "sample"),
            // Target::Pretest => write!(f, "pretest"),
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

/// 从字符串解析测试点ID集合
/// 支持格式：单个ID(1), 范围(1-5), 逗号分隔列表(1,3,5-7), all
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

        // 检查是否是范围
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
            // 单个ID
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

#[derive(Args, Debug)]
#[command(version, about = "数据生成工具")]
pub struct DmkArgs {
    /// 目标类型：data, sample, pretest
    #[arg(value_enum)]
    pub target: Target,

    /// 命令：gen, regen, reset
    #[arg(value_enum)]
    pub action: DmkCommand,

    #[arg(default_value = "all")]
    object: String,
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

    gen_data(&args, current_problem, current_day)?;

    Ok(())
}

fn gen_data(
    args: &DmkArgs,
    current_problem: &ProblemConfig,
    current_day: &ContestDayConfig,
) -> Result<(), anyhow::Error> {
    info!("开始生成数据: {}", current_problem.name);
    let target_dir = match args.target {
        Target::Data => current_problem.path.join("data"),
        Target::Sample => current_problem.path.join("sample"),
    };
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)?;
        info!("创建目标目录: {}", target_dir.display());
    }
    let generator_path = find_generator(&current_problem.path)?;
    let std_path = find_std(&current_problem)?;
    info!("找到生成器: {}", generator_path.display());
    info!("找到标程: {}", std_path.display());
    let (result1, result2) = rayon::join(
        || compile_generator(&generator_path),
        || compile_std(&std_path, current_problem, current_day),
    );
    result1?;
    result2?;
    let data_items: Vec<ExpandedDataItem> = match args.target {
        Target::Data => current_problem.data.iter().cloned().collect(),
        Target::Sample => current_problem
            .samples
            .iter()
            .map(|item| ExpandedDataItem {
                id: item.id,
                score: 0,   // 用不着
                subtest: 0, // 用不着
                input: item.input.get().unwrap().clone(),
                output: item.output.get().unwrap().clone(),
                args: item.args.clone(),
                manual: item.manual.unwrap_or(false),
            })
            .collect(),
    };
    let data_items: Vec<ExpandedDataItem> =
        data_items.into_iter().filter(|item| !item.manual).collect();
    let all_ids: Vec<u32> = data_items.iter().map(|data| data.id).collect();
    let target_ids = parse_test_object(&args.object, &all_ids)?;
    let data_items_to_gen: Vec<ExpandedDataItem> = data_items
        .into_iter()
        .filter(|item| target_ids.contains(&item.id))
        .collect();

    let seed = get_or_generate_seed(
        &target_dir,
        matches!(args.action, DmkCommand::Reset { .. }),
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

        let input_file = data_item.input;
        let output_file = data_item.output;

        let input_path = target_dir.join(input_file);
        let output_path = target_dir.join(output_file);

        if !matches!(args.action, DmkCommand::Gen { .. }) || !input_path.exists() {
            let mut args = current_problem.args.clone();
            args.extend(data_item.args.clone()); // 继承
            // 生成输入
            generate_input(&generator_path, &input_path, &seed, data_item.id, &args)?;
        }

        if !matches!(args.action, DmkCommand::Gen { .. }) || !output_path.exists() {
            // 使用标程生成输出
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
    save_seed(&target_dir, seed)?;
    Ok(())
}

/// 查找数据生成器
fn find_generator(problem_path: &Path) -> Result<PathBuf> {
    let path = problem_path.join("gen").join("gen.cpp");

    if path.exists() {
        return Ok(path);
    }

    bail!("未找到数据生成器文件")
}

/// 查找标程
fn find_std(problem: &ProblemConfig) -> Result<PathBuf> {
    for (name, case) in &problem.tests {
        if let ExpectedScore::Single(str) = &case.expected
            && str.replace(' ', "") == "==100"
            && problem.path.join(PathBuf::from(&case.path)).exists()
        {
            info!("找到标称 {name}, 位置 {}", case.path);
            return Ok(problem.path.join(PathBuf::from(&case.path)));
        }
    }

    bail!("未找到标程文件")
}

/// 编译生成器
fn compile_generator(generator_path: &Path) -> Result<()> {
    info!("编译数据生成器...");

    let output_path = generator_path.with_extension(std::env::consts::EXE_EXTENSION);

    let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));
    compile_pb.set_message("编译数据生成器");

    let status = Command::new("g++")
        .arg("-o")
        .arg(&output_path)
        .arg(generator_path)
        .arg("-O0") // 加速生成
        .arg("-std=c++17")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;

    compile_pb.finish_and_clear();

    if !status.status.success() {
        error!("{}", String::from_utf8(status.stderr)?);
        bail!("数据生成器编译失败");
    }

    info!("数据生成器编译成功");
    Ok(())
}

/// 编译标程
fn compile_std(std_path: &Path, problem: &ProblemConfig, day: &ContestDayConfig) -> Result<()> {
    info!("编译标程");

    let output_path = std_path.parent().unwrap().join(&problem.name);

    let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));
    compile_pb.set_message("编译标程");

    let status = build_compile_cmd(&std_path.to_path_buf(), &output_path, &day.compile)?
        .current_dir(&std_path.parent().unwrap())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    compile_pb.finish_and_clear();

    if !status.success() {
        bail!("标程编译失败");
    }

    info!("标程编译成功");
    Ok(())
}

/// 获取或生成种子
fn get_or_generate_seed(
    target_dir: &Path,
    force: bool,
    data: &Vec<ExpandedDataItem>,
) -> Result<BTreeMap<u32, u64>> {
    // 生成新种子

    let mut rng = gen_rnd()?;

    let mut seeds: BTreeMap<u32, u64> = BTreeMap::new();

    let seed_file = target_dir.join(".seed");

    if !force && seed_file.exists() {
        let seed_str = fs::read_to_string(&seed_file)?;

        seeds = serde_json::from_str(&seed_str).unwrap_or_else(|e| {
            warn!(".seed 文件无效, 重新生成: {}", e);
            BTreeMap::new()
        });
    }

    for i in data {
        let id = i.id;
        if !seeds.contains_key(&id) {
            seeds.insert(id, rng.random());
        }
    }
    return Ok(seeds);
}

/// 保存种子
fn save_seed(target_dir: &Path, seeds: BTreeMap<u32, u64>) -> Result<()> {
    let seed_file = target_dir.join(".seed");
    fs::write(seed_file, serde_json::to_string_pretty(&seeds)?)?;
    Ok(())
}

/// 生成输入文件
fn generate_input(
    generator_path: &Path,
    input_path: &Path,
    seeds: &BTreeMap<u32, u64>,
    test_id: u32,
    args: &HashMap<String, i64>,
) -> Result<()> {
    let generator_exe = generator_path.with_extension("");

    // 构建参数列表
    let mut cmd_args = vec![format!("{}", test_id)];

    // 添加自定义参数
    for (key, value) in args {
        cmd_args.push(format!("-{}={}", key, value.to_string()));
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
        bail!("生成器运行失败: {}", stderr);
    }

    // 写入输入文件
    fs::write(input_path, output.stdout)?;

    debug!("生成输入文件: {}", input_path.display());
    Ok(())
}

/// 使用标程生成输出文件
fn generate_output(
    std_path: &Path,
    input_path: &Path,
    output_path: &Path,
    problem_name: &str,
    file_io: bool,
) -> Result<()> {
    let std_exe = std_path.parent().unwrap().join(problem_name);

    if file_io {
        // 文件 IO 模式
        let work_dir = std_exe.parent().unwrap();
        let input_file = work_dir.join(format!("{}.in", problem_name));
        let output_file = work_dir.join(format!("{}.out", problem_name));

        // 复制输入文件
        fs::copy(input_path, &input_file)?;

        // 运行标程
        let status = Command::new(&std_exe)
            .current_dir(work_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()?;

        if !status.success() {
            bail!("标程运行失败");
        }

        // 复制输出文件
        fs::copy(&output_file, output_path)?;

        // 清理临时文件
        let _ = fs::remove_file(&input_file);
        let _ = fs::remove_file(&output_file);
    } else {
        // 标准 IO 模式
        let input_file = fs::File::open(input_path)?;
        let output_file = fs::File::create(output_path)?;

        let status = Command::new(&std_exe)
            .stdin(Stdio::from(input_file))
            .stdout(Stdio::from(output_file))
            .stderr(Stdio::piped())
            .status()?;

        if !status.success() {
            bail!("标程运行失败");
        }
    }

    debug!("生成输出文件: {}", output_path.display());
    Ok(())
}
