use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use clap::Args;
use clap::ValueEnum;
use indicatif::ProgressBar;
use owo_colors::OwoColorize;
use rand::Rng;

use crate::context::gctx;
use crate::prelude::*;
use crate::utils::random::gen_rnd;
use crate::utils::test_object::parse_test_object;
use crate::validate::compile_validator;
use tuack_lib::dmk::{DmkParams, DmkSession};
use tuack_lib::utils::testlib::{Generator, Validator};
use tuack_utils::compilers::cpp::CppRunner;
use tuack_utils::compilers::general::GeneralRunner;
use tuack_utils::compilers::generator::CppGenerator;
use tuack_utils::data::FsTestData;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
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

    /// 生成后校验输入（覆盖配置，如 --validate 或 --validate=false）
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
    validate: Option<bool>,
}

/// 数据点生成结果（展示状态）。
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

impl DmkResult {
    fn colored_status(&self) -> String {
        match self {
            DmkResult::Gen => "GEN".green().to_string(),
            DmkResult::Regen => "REGEN".green().bold().to_string(),
            DmkResult::Reset => "RESET".cyan().bold().to_string(),
            DmkResult::Skip => "SKIP".to_string(),
            DmkResult::Empty => "EMPTY".magenta().bold().to_string(),
            DmkResult::Fail(_) => "FAIL".red().bold().to_string(),
        }
    }

    fn error(&self) -> Option<&anyhow::Error> {
        match self {
            DmkResult::Fail(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DmkCommand> for DmkResult {
    fn from(action: DmkCommand) -> Self {
        match action {
            DmkCommand::Gen => DmkResult::Gen,
            DmkCommand::Regen => DmkResult::Regen,
            DmkCommand::Reset => DmkResult::Reset,
        }
    }
}

/// 打印单个数据点的生成状态
fn report_status(id: u32, kind: &str, status: &DmkResult) {
    msg_item!(
        status.colored_status(),
        "测试点 {} {}",
        id.to_string().cyan(),
        kind.bold()
    );
    if let Some(e) = status.error() {
        msg_error!("{}", e);
    }
}

/// 生成单个数据点的输入，返回展示状态。
async fn gen_input(
    session: &mut DmkSession<'_>,
    item: &FsTestData<'_>,
    seed: u64,
    action: DmkCommand,
) -> Result<DmkResult> {
    let exists = tokio::fs::try_exists(item.input_path())
        .await
        .unwrap_or(false);

    if !((!matches!(action, DmkCommand::Gen) || !exists) && item.gen_input()) {
        if exists {
            return Ok(DmkResult::Skip);
        }
        tokio::fs::write(item.input_path(), b"").await?;
        return Ok(DmkResult::Empty);
    }

    match session.gen_input(item, seed).await {
        Ok(()) => Ok(action.into()),
        Err(e) => Ok(DmkResult::Fail(e)),
    }
}

/// 用标程生成单个数据点的输出，返回展示状态。
async fn gen_output(
    session: &mut DmkSession<'_>,
    item: &FsTestData<'_>,
    action: DmkCommand,
) -> Result<DmkResult> {
    let exists = tokio::fs::try_exists(item.output_path())
        .await
        .unwrap_or(false);

    if !((!matches!(action, DmkCommand::Gen) || !exists) && item.gen_output()) {
        if exists {
            return Ok(DmkResult::Skip);
        }
        tokio::fs::write(item.output_path(), b"").await?;
        return Ok(DmkResult::Empty);
    }

    match session.gen_output(item).await {
        Ok(()) => Ok(action.into()),
        Err(e) => Ok(DmkResult::Fail(e)),
    }
}

/// 加载已有种子（文件不存在或无效均视为空）
async fn load_seeds(target_dir: &Path) -> BTreeMap<u32, u64> {
    let seed_file = target_dir.join(".seed");
    if let Ok(seed_str) = tokio::fs::read_to_string(&seed_file).await {
        serde_json::from_str(&seed_str).unwrap_or_else(|e| {
            msg_warn!(".seed 文件无效，重新生成：{}", e);
            BTreeMap::new()
        })
    } else {
        BTreeMap::new()
    }
}

/// 合并种子：`force`（Reset）时强制重新生成，否则只补缺失
fn merge_seeds(seeds: &mut BTreeMap<u32, u64>, items: &[FsTestData], force: bool) -> Result<()> {
    let mut rng = gen_rnd()?;
    for item in items {
        let id = item.id();
        if force {
            seeds.insert(id, rng.random::<u64>());
        } else {
            seeds.entry(id).or_insert_with(|| rng.random::<u64>());
        }
    }
    Ok(())
}

/// 保存种子
fn save_seed(target_dir: &Path, seeds: &BTreeMap<u32, u64>) -> Result<()> {
    let seed_file = target_dir.join(".seed");
    std::fs::write(seed_file, serde_json::to_string_pretty(seeds)?)?;
    Ok(())
}

/// 查找标程（tests 中期望得分 == 100 的文件）
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

/// 构造标程运行器
fn build_std_runner(
    std_path: &Path,
    day_config: &ContestDayConfig,
    problem_config: &ProblemConfig,
) -> Result<Box<dyn Runner>> {
    let mut runner: Box<dyn Runner> = match std_path
        .extension()
        .context("文件无后缀名")?
        .to_string_lossy()
        .to_string()
        .as_str()
    {
        "cpp" => Box::new(CppRunner::new(
            std_path,
            &day_config.compile,
            problem_config.name.clone(),
        )?),
        _ => Box::new(GeneralRunner::new(
            std_path,
            &day_config.compile,
            problem_config.name.clone(),
            &gctx().languages,
        )?),
    };

    if problem_config.problem_type == ProblemType::Interactive && runner.manifest().interactive {
        let interactive = problem_config.interactive.as_ref().unwrap();

        let resolve_path = |path: &String| -> Result<PathBuf> {
            let p = PathBuf::from_str(path)?;
            Ok(if p.is_absolute() {
                p
            } else {
                dunce::canonicalize(problem_config.path.join(p))?
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

    Ok(runner)
}

pub async fn main(args: DmkArgs) -> Result<()> {
    let config = gctx().config.as_ref().context("没有找到有效的工程")?;

    let (current_problem, current_day) =
        if let CurrentLocation::Problem(ref day, ref prog) = config.location {
            let day_config = config
                .config
                .subconfig
                .get(day)
                .context(format!("无法获取天配置：{}", day))?;

            let problem_config = day_config
                .subconfig
                .get(prog)
                .context(format!("无法获取题目配置：{}/{}", day, prog))?;

            (problem_config, day_config)
        } else {
            bail!("本命令只能在题目目录下执行");
        };

    let selected: Vec<FsTestData> = match &args.target {
        Target::Data => tuack_utils::data::problem_test_data(current_problem),
        Target::Sample => tuack_utils::data::problem_sample_data(current_problem),
    };
    let selected = parse_test_object(&args.object, &selected, FsTestData::id)?;

    let generator_config = match &args.target {
        Target::Data => current_problem
            .generator
            .as_ref()
            .context("generator 未配置")?
            .data
            .clone(),
        Target::Sample => {
            let pair = current_problem
                .generator
                .as_ref()
                .context("generator 未配置")?;
            pair.sample.clone().unwrap_or(pair.data.clone())
        }
    };

    let resolve = |path: &str| current_problem.path.join(path);

    let gen_path = resolve(&generator_config.source);

    let mut deps: IndexMap<String, Vec<u8>> = IndexMap::new();
    for dep_path in &generator_config.deps {
        let abs = resolve(dep_path);
        let content =
            std::fs::read(&abs).with_context(|| format!("读取依赖文件失败：{}", abs.display()))?;
        let name = abs
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        deps.insert(name, content);
    }

    // 编译数据生成器
    let gen_compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    gen_compile_pb.enable_steady_tick(Duration::from_millis(100));
    gen_compile_pb.set_message("编译数据生成器");
    let mut generator = CppGenerator::new(&gen_path, &current_day.compile, deps)?;
    let gen_result = generator.prepare();
    gen_compile_pb.finish_and_clear();
    gen_result.context("数据生成器编译失败")?;

    let effective_validate = args.validate.unwrap_or(generator_config.validate);
    let validator: Option<Box<dyn Validator>> = if effective_validate {
        Some(
            compile_validator(current_problem, args.target.into())
                .with_context(|| format!("题目 {} 的 Validator 不可用", current_problem.name))?,
        )
    } else {
        None
    };

    let target_dir = match &args.target {
        Target::Data => current_problem.path.join("data"),
        Target::Sample => current_problem.path.join("sample"),
    };
    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
        info!("创建目标目录：{}", target_dir.display());
    }

    // 种子：加载 -> 合并（Reset 强制重生成）-> 结束时保存
    let mut seeds = load_seeds(&target_dir).await;
    merge_seeds(
        &mut seeds,
        &selected,
        matches!(args.action, DmkCommand::Reset),
    )?;

    if selected.is_empty() {
        msg_warn!("没有需要生成的数据");
        return Ok(());
    }

    // 标程：查找 -> 构造 -> 交互配置 -> 编译
    let std_path = find_std(current_problem)?;
    info!("找到标程：{}", std_path.display());
    let mut runner = build_std_runner(&std_path, current_day, current_problem)?;

    let std_compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    std_compile_pb.enable_steady_tick(Duration::from_millis(100));
    std_compile_pb.set_message("编译标程");
    let std_result = runner.prepare_async().await;
    std_compile_pb.finish_and_clear();
    std_result?;

    let params = DmkParams {
        problem_name: current_problem.name.clone(),
        file_io: current_problem.file_io.unwrap_or(true),
    };
    let mut session = DmkSession::new(&mut *runner, &mut generator, validator.as_deref(), params);

    let dmk_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(selected.len() as u64));
    dmk_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.cyan/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    for item in &selected {
        dmk_pb.set_message(format!(
            "{}/{} | 正在生成数据点 #{}",
            dmk_pb.position(),
            dmk_pb.length().unwrap(),
            item.id()
        ));

        let seed = *seeds.get(&item.id()).unwrap();
        let status = gen_input(&mut session, item, seed, args.action).await?;
        report_status(item.id(), "输入", &status);

        if matches!(status, DmkResult::Fail(_)) {
            report_status(item.id(), "输出", &DmkResult::Skip);
        } else {
            let status = gen_output(&mut session, item, args.action).await?;
            report_status(item.id(), "输出", &status);
        }

        dmk_pb.inc(1);
    }

    dmk_pb.finish_with_message("数据生成完成！");

    save_seed(&target_dir, &seeds)?;

    Ok(())
}
