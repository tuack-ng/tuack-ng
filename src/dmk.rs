use crate::config::ExpandedDataItem;
use crate::context::{CurrentLocation, gctx};
use crate::prelude::*;
use crate::tuack_lib::dmk::{DmkReporter, DmkResult, dmk};
use crate::utils::compilers::generator::CppGenerator;
use crate::utils::test_object::parse_test_object;
use clap::Args;
use clap::ValueEnum;
use indicatif::ProgressBar;
use owo_colors::OwoColorize;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

struct CliDmkReporter {
    std_compile_pb: ProgressBar,
    dmk_compile_pb: ProgressBar,
    dmk_pb: ProgressBar,
}

impl CliDmkReporter {
    fn new() -> Self {
        Self {
            std_compile_pb: gctx().multiprogress.add(ProgressBar::new_spinner()),
            dmk_compile_pb: gctx().multiprogress.add(ProgressBar::new_spinner()),
            dmk_pb: gctx().multiprogress.add(ProgressBar::new(0)),
        }
    }
}

impl DmkReporter for CliDmkReporter {
    fn compiling_dmk(&self) {
        self.dmk_compile_pb
            .enable_steady_tick(Duration::from_millis(100));
        self.dmk_compile_pb.set_message("编译数据生成器");
    }

    fn compiled_dmk(&self) {
        self.dmk_compile_pb.finish_and_clear();
    }

    fn compiling_std(&self) {
        self.std_compile_pb
            .enable_steady_tick(Duration::from_millis(100));
        self.std_compile_pb.set_message("编译标程");
    }

    fn compiled_std(&self) {
        self.std_compile_pb.finish_and_clear();
    }

    fn start_dmk(&self, size: u32) {
        self.dmk_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.cyan/blue}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
        self.dmk_pb.set_length(size as u64);
    }

    fn start_item(&self, id: u32) {
        self.dmk_pb.set_message(format!(
            "{}/{} | 正在生成数据点 #{}",
            self.dmk_pb.position(),
            self.dmk_pb.length().unwrap(),
            id
        ));
    }

    fn generate_input(&self, id: u32, status: &DmkResult) {
        msg_item!(
            status.colored_status(),
            "测试点 {} {}",
            id.to_string().cyan(),
            "输入".bold()
        );
        if let Some(e) = status.error() {
            msg_error!("{}", e);
        }
    }

    fn generate_output(&self, id: u32, status: &DmkResult) {
        msg_item!(
            status.colored_status(),
            "测试点 {} {}",
            id.to_string().cyan(),
            "输出".bold()
        );
        if let Some(e) = status.error() {
            msg_error!("{}", e);
        }
    }

    fn progress(&self, position: u32) {
        self.dmk_pb.set_position((position + 1) as u64);
    }

    fn completed(&self) {
        self.dmk_pb.finish_with_message("数据生成完成！");
    }
}

#[derive(Debug, Clone, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmkCommand {
    /// 生成（未生成的）数据
    Gen,
    /// 重新生成数据（使用相同种子）
    Regen,
    /// 重置种子
    Reset,
}

#[derive(Args, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

use crate::tuack_lib::dmk as tuack_dmk;

// 转换 Target
impl From<Target> for tuack_dmk::Target {
    fn from(value: Target) -> Self {
        match value {
            Target::Data => tuack_dmk::Target::Data,
            Target::Sample => tuack_dmk::Target::Sample,
        }
    }
}

// 转换 DmkCommand
impl From<DmkCommand> for tuack_dmk::DmkCommand {
    fn from(value: DmkCommand) -> Self {
        match value {
            DmkCommand::Gen => tuack_dmk::DmkCommand::Gen,
            DmkCommand::Regen => tuack_dmk::DmkCommand::Regen,
            DmkCommand::Reset => tuack_dmk::DmkCommand::Reset,
        }
    }
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

    let data_items: Vec<ExpandedDataItem> = match &args.target {
        Target::Data => current_problem.runtime.data.clone(),
        Target::Sample => current_problem
            .runtime
            .samples
            .iter()
            .map(|item| ExpandedDataItem {
                id: item.id,
                score: 0,
                subtask: 0,
                input: item.input.clone(),
                output: item.output.clone(),
                orig_args: item.args.clone(),
                args: item.args.clone(),
                dmk: item.dmk,
            })
            .collect(),
    };

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

    let mut generator = CppGenerator::new(&gen_path, &current_day.compile, "gen", deps)?;

    let effective_validate = args.validate.unwrap_or(generator_config.validate);
    let validator: Option<Box<dyn crate::tuack_lib::utils::testlib::Validator>> =
        if effective_validate {
            Some(
                crate::validate::compile_validator(current_problem, args.target.into())
                    .with_context(|| {
                        format!("题目 {} 的 Validator 不可用", current_problem.name)
                    })?,
            )
        } else {
            None
        };

    let reporter = CliDmkReporter::new();

    dmk(
        &reporter,
        &args.target.into(),
        &args.action.into(),
        &parse_test_object(&args.object, &data_items)?,
        current_problem,
        current_day,
        &mut generator,
        validator.as_deref(),
    )
    .await
}
