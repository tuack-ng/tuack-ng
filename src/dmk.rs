use crate::context::{CurrentLocation, gctx};
use crate::prelude::*;
use crate::tuack_lib::config::ExpandedDataItem;
use crate::utils::compile::{build_compile_cmd, build_run_cmd};
use crate::utils::filesystem::create_or_clear_dir;
use crate::utils::random::gen_rnd;
use clap::Args;
use clap::ValueEnum;
use colored::ColoredString;
use indicatif::ProgressBar;
use rand::Rng;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::process::Command;
use tokio::sync::mpsc;

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
#[command(version)]
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

#[derive(Debug)]
pub enum DmkStatus {
    /// 正在编译 Std
    CompilingStd,
    /// 正在编译 Dmk
    CompilingDmk,
    /// Std 完成编译
    CompiledStd,
    /// Dmk 完成编译
    CompiledDmk,

    /// 开始生成数据，并报告总数
    StartDmk(u32),
    /// 生成输入
    DmkInput {
        id: u32,
        status: ColoredString,
        error: Option<anyhow::Error>,
    },
    /// 生成输出
    DmkOutput {
        id: u32,
        status: ColoredString,
        error: Option<anyhow::Error>,
    },
    /// 报告生成进度
    DmkStart(u32),
    DmkProgress(u32),

    /// 完成
    Completed,
}

/// 从字符串解析测试点，返回匹配的 ExpandedDataItem 列表
pub fn parse_test_object(
    s: &str,
    all_items: &[Arc<ExpandedDataItem>],
) -> Result<Vec<Arc<ExpandedDataItem>>> {
    let s = s.trim().to_lowercase();

    if s == "all" {
        return Ok(all_items.to_vec());
    }

    let mut result = Vec::new();
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
                .with_context(|| format!("无效的起始ID: {}", start_str))?;
            let end = end_str
                .parse::<u32>()
                .with_context(|| anyhow!("无效的结束ID: {}", end_str))?;

            if start > end {
                bail!("起始ID不能大于结束ID: {}", part);
            }

            // 遍历查找在范围内的测试点
            for item in all_items.iter() {
                if item.id >= start && item.id <= end {
                    result.push(item.clone());
                }
            }
        } else {
            let id = part
                .parse::<u32>()
                .with_context(|| anyhow!("无效的测试点ID: {}", part))?;

            // 遍历查找匹配的测试点
            if let Some(item) = all_items.iter().find(|item| item.id == id) {
                result.push(item.clone());
            }
        }
    }

    Ok(result)
}

pub async fn main(args: DmkArgs) -> Result<()> {
    let config = gctx().config.as_ref().context("没有找到有效的工程")?;

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

    let data_items: Vec<Arc<ExpandedDataItem>> = match &args.target {
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

    let (tx, mut rx) = mpsc::channel::<DmkStatus>(10);

    let gen_handle = tokio::spawn(async move {
        gen_data(
            tx,
            &args.target,
            &args.action,
            &parse_test_object(&args.object, &data_items)?,
            current_problem,
            current_day,
        )
        .await
    });

    let std_compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    let dmk_compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());

    let dmk_pb = gctx().multiprogress.add(ProgressBar::new(0));

    while let Some(status) = rx.recv().await {
        match status {
            DmkStatus::CompilingDmk => {
                dmk_compile_pb.enable_steady_tick(Duration::from_millis(100));
                dmk_compile_pb.set_message("编译数据生成器");
            }
            DmkStatus::CompiledDmk => {
                dmk_compile_pb.finish_and_clear();
            }
            DmkStatus::CompilingStd => {
                std_compile_pb.enable_steady_tick(Duration::from_millis(100));
                std_compile_pb.set_message("编译标程");
            }
            DmkStatus::CompiledStd => {
                std_compile_pb.finish_and_clear();
            }
            DmkStatus::StartDmk(size) => {
                dmk_pb.set_style(
                    indicatif::ProgressStyle::default_bar()
                        .template("  [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                        .unwrap()
                        .progress_chars("=> "),
                );
                dmk_pb.set_length(size as u64);
            }
            DmkStatus::DmkInput { id, status, error } => {
                msg_item!(status, "测试点 {} {}", id.to_string().cyan(), "输入".bold());
                if let Some(e) = error {
                    msg_error!("{}", e);
                }
            }
            DmkStatus::DmkOutput { id, status, error } => {
                msg_item!(status, "测试点 {} {}", id.to_string().cyan(), "输出".bold());
                if let Some(e) = error {
                    msg_error!("{}", e);
                }
            }
            DmkStatus::DmkProgress(progress) => dmk_pb.set_position(progress as u64),
            DmkStatus::DmkStart(progress) => {
                dmk_pb.set_message(format!("生成数据点 #{}", progress))
            }
            DmkStatus::Completed => {
                dmk_pb.finish_with_message("数据生成完成！");
            }
        }
    }

    gen_handle.await?
}

async fn gen_data(
    tx: mpsc::Sender<DmkStatus>,
    target: &Target,
    action: &DmkCommand,
    data_items: &Vec<Arc<ExpandedDataItem>>,
    current_problem: &ProblemConfig,
    current_day: &ContestDayConfig,
) -> Result<()> {
    info!("开始生成数据: {}", current_problem.name);
    let target_dir = match target {
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

    let generator_path_clone = generator_path.clone();
    let std_path_clone = std_path.clone();
    let current_problem_clone = current_problem.clone();
    let current_day_clone = current_day.clone();
    let tx_clone1 = tx.clone();
    let tx_clone2 = tx.clone();

    let (result1, result2) = tokio::join!(
        compile_generator(tx_clone1, &generator_path_clone),
        compile_std(
            tx_clone2,
            &std_path_clone,
            &current_problem_clone,
            &current_day_clone
        )
    );
    if let Err(e) = result1 {
        // msg_error!("数据生成器编译错误: {}", e);
        bail!(e.context("数据生成器编译失败"))
    }

    if let Err(e) = result2 {
        // msg_error!("标程编译错误: {}", e);
        bail!(e.context("标程编译失败"))
    }

    let seeds = get_or_generate_seed(
        &target_dir,
        matches!(action, DmkCommand::Reset),
        &data_items,
    )
    .await?;

    if data_items.is_empty() {
        msg_warn!("没有需要生成的数据");
        return Ok(());
    }

    tx.send(DmkStatus::StartDmk(data_items.len() as u32))
        .await?;

    let action_str = match action {
        DmkCommand::Gen => "GEN".green(),
        DmkCommand::Regen => "REGEN".green().bold(),
        DmkCommand::Reset => "RESET".cyan().bold(),
    };

    for (id, data_item) in data_items.iter().enumerate() {
        tx.send(DmkStatus::DmkStart(data_item.id as u32)).await?;

        let input_file = data_item.input.clone();
        let output_file = data_item.output.clone();

        let input_path = target_dir.join(&input_file);
        let output_path = target_dir.join(&output_file);

        if !matches!(action, DmkCommand::Gen) || !input_path.exists() {
            let mut args_map = current_problem.args.clone();
            args_map.extend(data_item.args.clone());

            if let Err(e) = generate_input(
                &generator_path,
                &input_path,
                &seeds,
                data_item.id,
                &args_map,
            )
            .await
            {
                tx.send(DmkStatus::DmkInput {
                    id: data_item.id as u32,
                    status: "FAIL".red().bold(),
                    error: Some(e),
                })
                .await?;
            } else {
                tx.send(DmkStatus::DmkInput {
                    id: data_item.id as u32,
                    status: action_str.clone(),
                    error: None,
                })
                .await?;
            }
        } else {
            tx.send(DmkStatus::DmkInput {
                id: data_item.id as u32,
                status: "SKIP".into(),
                error: None,
            })
            .await?;
        }

        if !matches!(action, DmkCommand::Gen) || !output_path.exists() {
            if let Err(e) = generate_output(
                &std_path,
                &input_path,
                &output_path,
                &current_problem.name,
                current_problem.file_io.unwrap_or(true),
            )
            .await
            {
                tx.send(DmkStatus::DmkOutput {
                    id: data_item.id as u32,
                    status: "FAIL".red().bold(),
                    error: Some(e),
                })
                .await?;
            } else {
                tx.send(DmkStatus::DmkOutput {
                    id: data_item.id as u32,
                    status: action_str.clone(),
                    error: None,
                })
                .await?;
            }
        } else {
            tx.send(DmkStatus::DmkOutput {
                id: data_item.id as u32,
                status: "SKIP".into(),
                error: None,
            })
            .await?;
        }

        tx.send(DmkStatus::DmkProgress(id as u32)).await?;
    }

    let _ = std::fs::remove_dir_all(std_path.parent().unwrap().join("tmp"));
    save_seed(&target_dir, seeds)?;
    tx.send(DmkStatus::Completed).await?;

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
fn find_std(problem: &crate::tuack_lib::config::ProblemConfig) -> Result<PathBuf> {
    for (name, case) in &problem.tests {
        if let crate::tuack_lib::config::ExpectedScore::Single(str) = &case.expected
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
async fn compile_generator(tx: mpsc::Sender<DmkStatus>, generator_path: &Path) -> Result<()> {
    info!("编译数据生成器");

    let tmp_dir = generator_path.parent().unwrap();

    tx.send(DmkStatus::CompilingDmk).await?;

    let output_path = tmp_dir.join("gen");

    let status = Command::new("g++")
        .arg("-o")
        .arg(&output_path)
        .arg(generator_path)
        .arg("-O2")
        .arg("-std=c++17")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !status.status.success() {
        bail!("{}", String::from_utf8_lossy(&status.stderr));
    }

    info!("数据生成器编译成功");

    tx.send(DmkStatus::CompiledDmk).await?;

    Ok(())
}

/// 编译标程
async fn compile_std(
    tx: mpsc::Sender<DmkStatus>,
    std_path: &Path,
    problem: &crate::tuack_lib::config::ProblemConfig,
    day: &crate::tuack_lib::config::ContestDayConfig,
) -> Result<()> {
    info!("编译标程: {}", std_path.display());

    let tmp_dir = std_path.parent().unwrap().join("tmp");
    create_or_clear_dir(&tmp_dir)?;

    let src_path = tmp_dir.join(std_path.file_name().unwrap());
    fs::copy(std_path, &src_path).await?;

    let program_name = problem.name.clone();

    let compile_cmd = build_compile_cmd(&src_path, &tmp_dir, &program_name, &day.compile)?;

    tx.send(DmkStatus::CompilingStd).await?;

    if let Some(mut cmd) = compile_cmd {
        let output = cmd
            .current_dir(&tmp_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            bail!("{}", String::from_utf8_lossy(&output.stderr));
        }

        info!("标程编译成功");
    } else {
        // 对于无需编译的语言，复制源文件
        let target_path = tmp_dir
            .join(&program_name)
            .with_extension(std_path.extension().unwrap_or_default());
        std::fs::copy(&src_path, &target_path)?;
        info!("标程准备完成");
    }

    tx.send(DmkStatus::CompiledStd).await?;

    Ok(())
}

/// 获取或生成种子
async fn get_or_generate_seed(
    target_dir: &Path,
    force: bool,
    data: &[Arc<ExpandedDataItem>],
) -> Result<BTreeMap<u32, u64>> {
    let mut rng = gen_rnd()?;
    let mut seeds: BTreeMap<u32, u64> = BTreeMap::new();

    let seed_file = target_dir.join(".seed");

    if !force && seed_file.exists() {
        let seed_str = fs::read_to_string(&seed_file).await?;
        seeds = serde_json::from_str(&seed_str).unwrap_or_else(|e| {
            msg_warn!(".seed 文件无效, 重新生成: {}", e);
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
fn save_seed(target_dir: &Path, seeds: BTreeMap<u32, u64>) -> Result<()> {
    let seed_file = target_dir.join(".seed");
    std::fs::write(seed_file, serde_json::to_string_pretty(&seeds)?)?;
    Ok(())
}

/// 生成输入文件
async fn generate_input(
    generator_path: &Path,
    input_path: &Path,
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
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("生成器运行失败 (测试点 {}): {}", test_id, stderr);
    }

    // 写入输入文件
    fs::write(input_path, &output.stdout).await?;

    debug!("生成输入文件: {}", input_path.display(),);
    Ok(())
}

/// 使用标程生成输出文件
async fn generate_output(
    std_path: &Path,
    input_path: &Path,
    output_path: &Path,
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

    fs::copy(input_path, &work_input_path).await?;
    debug!("复制输入文件到工作目录: {}", work_input_path.display());

    // 准备输出路径
    let work_output_path = if file_io {
        work_dir.join(format!("{}.out", problem_name))
    } else {
        work_dir.join(format!("{}.stdout", problem_name))
    };

    let mut cmd = if let Some(cmd) = build_run_cmd(std_path, work_dir, problem_name)? {
        Command::from(cmd)
    } else {
        let exe_extension = std::env::consts::EXE_EXTENSION;
        let executable_path = work_dir.join(problem_name).with_extension(exe_extension);

        if !executable_path.exists() {
            bail!("找不到可执行文件: {}", executable_path.display());
        }

        debug!("使用可执行文件: {}", executable_path.display());
        Command::new(executable_path)
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
    let output = child.output().await?;

    if !output.status.success() {
        if !output.stderr.is_empty() {
            bail!(
                "标程运行失败，退出码: {}\n标准错误输出: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            bail!("标程运行失败，退出码: {}", output.status);
        }
    }

    // 检查输出文件是否生成
    if !work_output_path.exists() {
        bail!("标程未生成输出文件: {}", work_output_path.display());
    }

    // 复制输出文件到目标位置
    fs::copy(&work_output_path, output_path).await?;

    debug!("成功生成输出文件: {}", output_path.display());

    info!("标程成功生成输出");
    Ok(())
}
