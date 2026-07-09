use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use rand::Rng;
use tokio::fs;

use crate::prelude::*;
use crate::config::ExpandedDataItem;
use crate::tuack_lib::utils::testlib::Generator;
use crate::utils::compilers::cpp::CppRunner;
use crate::utils::compilers::general::GeneralRunner;
use crate::utils::random::gen_rnd;

/// 数据生成报告器接口
/// [`dmk()`] 接收一个实现了此 trait 的对象
pub trait DmkReporter: Send + Sync {
    /// 正在编译 Dmk
    fn compiling_dmk(&self);
    /// 完成编译 Dmk
    fn compiled_dmk(&self);
    /// 正在编译 Std
    fn compiling_std(&self);
    /// 完成编译 Std
    fn compiled_std(&self);
    /// 开始生成数据
    ///
    /// # 参数
    /// - `size`: 需要生成的总数
    fn start_dmk(&self, size: u32);
    /// 开始生成数据点
    ///
    /// # 参数
    /// - `id`: 数据点编号
    fn start_item(&self, id: u32);
    /// 生成数据点的输入
    ///
    /// # 参数
    /// - `id`: 数据点编号
    /// - `status`: 数据点生成结果
    fn generate_input(&self, id: u32, status: &DmkResult);
    /// 生成数据点的输出
    ///
    /// # 参数
    /// - `id`: 数据点编号
    /// - `status`: 数据点生成结果
    fn generate_output(&self, id: u32, status: &DmkResult);
    /// 数据点生成进度
    ///
    /// # 参数
    /// - `position`: 进度，相对于 [`start_dmk()`] 的 `id` 而言。
    ///
    /// [`start_dmk()`]: Self::start_dmk
    fn progress(&self, position: u32);
    /// 生成完成
    fn completed(&self);
}

#[derive(Debug)]
pub enum DmkResult {
    /// 生成数据
    Gen,
    /// 重新生成数据
    Regen,
    /// 重置种子并重新生成数据
    Reset,
    /// 跳过
    Skip,
    /// 建造空文件
    Empty,
    /// 失败
    Fail(anyhow::Error),
}

impl From<&DmkCommand> for DmkResult {
    fn from(action: &DmkCommand) -> Self {
        match action {
            DmkCommand::Gen => DmkResult::Gen,
            DmkCommand::Regen => DmkResult::Regen,
            DmkCommand::Reset => DmkResult::Reset,
        }
    }
}

#[derive(Debug, Clone)]
pub enum DmkCommand {
    /// 生成（未生成的）数据
    Gen,
    /// 重新生成数据（使用相同种子）
    Regen,
    /// 重置种子
    Reset,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// 正式测试数据
    Data,
    /// 样例数据
    Sample,
}

pub async fn dmk(
    reporter: &dyn DmkReporter,
    target: &Target,
    action: &DmkCommand,
    data_items: &[Arc<ExpandedDataItem>],
    current_problem: &ProblemConfig,
    current_day: &ContestDayConfig,
    generator: &mut impl Generator,
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
    let std_path = find_std(current_problem)?;
    let mut runner: Box<dyn Runner> = match std_path
        .extension()
        .context("文件无后缀名")?
        .to_string_lossy()
        .to_string()
        .as_str()
    {
        "cpp" => Box::new(CppRunner::new(
            &std_path,
            &current_day.compile,
            current_problem.name.clone(),
        )?),
        _ => Box::new(GeneralRunner::new(
            &std_path,
            &current_day.compile,
            current_problem.name.clone(),
        )?),
    };
    info!("找到标程: {}", std_path.display());

    if current_problem.problem_type == ProblemType::Interactive
        && runner.manifest().interactive {
            let interactive = current_problem.interactive.as_ref().unwrap();

            let resolve_path = |path: &String| -> Result<PathBuf> {
                let p = PathBuf::from_str(path)?;
                Ok(if p.is_absolute() {
                    p
                } else {
                    dunce::canonicalize(current_problem.path.join(p))?
                })
            };
            let grader_path = match &interactive.dmk_grader {
                Some(dmk_grader) => resolve_path(dmk_grader)?,
                None => resolve_path(&interactive.grader)?,
            };
            let header_path = resolve_path(&interactive.header)?;

            if !grader_path.exists() {
                bail!("grader 不存在")
            }
            if !header_path.exists() {
                bail!("header 不存在")
            }

            runner.set_interactive(&grader_path, &header_path)?;
        }

    reporter.compiling_dmk();
    generator.prepare().context("数据生成器编译失败")?;
    reporter.compiled_dmk();

    compile_std(reporter, &mut runner).await?;

    let seeds =
        get_or_generate_seed(&target_dir, matches!(action, DmkCommand::Reset), data_items).await?;

    if data_items.is_empty() {
        msg_warn!("没有需要生成的数据");
        return Ok(());
    }

    reporter.start_dmk(data_items.len() as u32);

    for (id, data_item) in data_items.iter().enumerate() {
        reporter.start_item(data_item.id);

        let input_file = data_item.input.clone();
        let output_file = data_item.output.clone();

        let input_path = target_dir.join(&input_file);
        let output_path = target_dir.join(&output_file);

        let gen_input = data_item.dmk == DmkConfig::Input || data_item.dmk == DmkConfig::On;
        let gen_output = data_item.dmk == DmkConfig::Output || data_item.dmk == DmkConfig::On;

        if (!matches!(action, DmkCommand::Gen) || !input_path.exists()) && gen_input {
            let mut args_map = current_problem.args.clone();
            args_map.extend(data_item.args.clone());

            let seed = seeds.get(&data_item.id).unwrap();

            match generator.run(args_map, *seed) {
                Ok(output) => {
                    if let Err(e) = fs::write(&input_path, &output).await {
                        reporter.generate_input(data_item.id, &DmkResult::Fail(e.into()));
                    } else {
                        reporter.generate_input(data_item.id, &action.into());
                    }
                }
                Err(e) => {
                    reporter.generate_input(data_item.id, &DmkResult::Fail(e));
                }
            }
        } else {
            if !input_path.exists() {
                fs::write(&input_path, b"").await?;
                reporter.generate_input(data_item.id, &DmkResult::Empty);
            } else {
                reporter.generate_input(data_item.id, &DmkResult::Skip);
            }
        }

        if (!matches!(action, DmkCommand::Gen) || !output_path.exists()) && gen_output {
            if let Err(e) = generate_output(
                &mut runner,
                // &std_path,
                &input_path,
                &output_path,
                &current_problem.name,
                current_problem.file_io.unwrap_or(true),
            )
            .await
            {
                reporter.generate_output(data_item.id, &DmkResult::Fail(e));
            } else {
                reporter.generate_output(data_item.id, &action.into());
            }
        } else {
            if !output_path.exists() {
                fs::write(&output_path, b"").await?;
                reporter.generate_output(data_item.id, &DmkResult::Empty);
            } else {
                reporter.generate_output(data_item.id, &DmkResult::Skip);
            }
        }

        reporter.progress(id as u32);
    }

    let _ = std::fs::remove_dir_all(std_path.parent().unwrap().join("tmp"));
    save_seed(&target_dir, seeds)?;
    reporter.completed();

    Ok(())
}

/// 查找标程
fn find_std(problem: &crate::config::ProblemConfig) -> Result<PathBuf> {
    for (name, case) in &problem.tests {
        if let crate::config::ExpectedScore::Single(str) = &case.expected
            && str.replace(' ', "") == "==100"
            && problem.path.join(PathBuf::from(&case.path)).exists()
        {
            info!("找到标称 {name}, 位置 {}", case.path);
            return Ok(problem.path.join(PathBuf::from(&case.path)));
        }
    }

    bail!("未找到标程文件")
}

/// 编译标程
async fn compile_std(
    reporter: &dyn DmkReporter,
    // std_path: &Path,
    runner: &mut Box<dyn Runner>,
) -> Result<()> {
    info!("编译标程");

    reporter.compiling_std();

    runner.prepare_async().await?;

    reporter.compiled_std();

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

/// 使用标程生成输出文件
async fn generate_output(
    runner: &mut Box<dyn Runner>,
    input_path: &Path,
    output_path: &Path,
    problem_name: &str,
    file_io: bool,
) -> Result<()> {
    let input_bytes = tokio::fs::read(input_path).await?;
    runner.set_input(input_bytes);

    if file_io {
        runner.set_io_mode(IoMode::File {
            input_name: format!("{}.in", problem_name),
            output_name: format!("{}.out", problem_name),
        });
    } else {
        runner.set_io_mode(IoMode::Stdio);
    }

    runner.set_limits(ResourceLimits::unlimited());

    let result = runner.execute().await?;

    match result.status {
        RunStatus::Success => {}
        _ if !result.stderr.is_empty() => {
            bail!(
                "标程运行失败\n标准错误输出: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        _ => {
            bail!("标程运行失败");
        }
    }

    if result.output.is_empty() {
        bail!("标程未生成输出");
    }

    tokio::fs::write(output_path, &result.output).await?;

    debug!("成功生成输出文件: {}", output_path.display());

    info!("标程成功生成输出");
    Ok(())
}
