use clap::{Args, ValueEnum};
use indicatif::ProgressBar;
use owo_colors::OwoColorize;
use std::time::Duration;

use crate::config::ExpandedDataItem;
use crate::data::fs::FsTestData;
use crate::prelude::*;
use crate::tuack_lib::data::Data;
use crate::tuack_lib::utils::testlib::{Validator, ValidatorResult};
use crate::utils::validators::cpp::CppValidator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// 正式测试数据
    Data,
    /// 样例数据
    Sample,
}

#[derive(Args, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[command(version)]
pub struct ValidateArgs {
    /// 目标类型
    #[arg(value_enum, default_value = "data")]
    pub target: Target,

    /// 校验对象，使用 `,` 和 `-` 分割 (如 1,2-3,4-10)
    #[arg(default_value = "all")]
    object: String,
}

/// 编译 Validator，失败时返回错误，由调用方决定如何输出
pub fn compile_validator(
    problem_config: &ProblemConfig,
    target: Target,
) -> Result<Box<dyn Validator>> {
    let is_sample = matches!(target, Target::Sample);

    let val_config = match &problem_config.validator {
        Some(pair) => {
            if is_sample {
                pair.sample.as_ref().unwrap_or(&pair.data)
            } else {
                &pair.data
            }
        }
        None => bail!("Validator 未配置"),
    };

    let resolve = |path: &str| problem_config.path.join(path);
    let source_path = resolve(&val_config.source);

    if !source_path.exists() {
        bail!("Validator 不存在：{}", source_path.display());
    }

    let mut deps: IndexMap<String, Vec<u8>> = IndexMap::new();
    for dep_path in &val_config.deps {
        let abs = resolve(dep_path);
        let content =
            fs::read(&abs).with_context(|| format!("Validator 依赖读取失败：{}", abs.display()))?;
        let name = abs.file_name().unwrap().to_string_lossy().to_string();
        deps.insert(name, content);
    }

    let compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
    compile_pb.enable_steady_tick(Duration::from_millis(100));
    compile_pb.set_message(format!("编译 {} 题目的 Validator", problem_config.name));

    let result = (|| -> Result<Box<dyn Validator>> {
        let mut cpp_validator = CppValidator::new(&source_path, &IndexMap::new(), "val", deps)
            .context("Validator 初始化失败")?;
        cpp_validator.prepare().context("Validator 编译失败")?;
        Ok(Box::new(cpp_validator) as Box<dyn Validator>)
    })();

    compile_pb.finish_and_clear();
    result
}

async fn validate_problem(
    problem_config: &ProblemConfig,
    target: Target,
    object: &str,
    in_problem: bool,
) -> Result<()> {
    let selected: Vec<ExpandedDataItem> = match target {
        Target::Data => problem_config.runtime.data.clone(),
        Target::Sample => problem_config
            .samples
            .iter()
            .map(|item| ExpandedDataItem {
                id: item.id,
                score: 1,
                subtask: 0,
                input: item.input_path(),
                output: item.output_path(),
                orig_args: item.args.clone(),
                args: item.args.clone(),
                dmk: item.dmk.unwrap_or(problem_config.dmk),
            })
            .collect(),
    };

    let selected = crate::utils::test_object::parse_test_object(object, &selected)?;

    let data_items: Vec<FsTestData<'_>> = match target {
        Target::Data => problem_config.test_data(),
        Target::Sample => problem_config.sample_data(),
    };
    let data_items = data_items
        .into_iter()
        .filter(|item| selected.iter().any(|sel| sel.id == item.id()))
        .collect::<Vec<_>>();

    let validator = match compile_validator(problem_config, target) {
        Ok(v) => v,
        Err(e) => {
            msg_error!(
                "题目 {} 的 Validator 不可用：{:#}",
                problem_config.name.magenta(),
                e
            );
            return Ok(());
        }
    };

    let case_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(data_items.len() as u64));
    case_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.magenta/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut failed = 0;
    for (idx, data_item) in data_items.iter().enumerate() {
        let result = match data_item.input().await {
            Ok(mut reader) => validator.validate(&mut *reader).await,
            Err(e) => {
                msg_item!(
                    "FAIL".red().bold(),
                    "测试点 {} 读取输入失败：{}",
                    data_item.id().to_string().bold(),
                    e
                );
                failed += 1;
                case_pb.inc(1);
                continue;
            }
        };

        match result {
            Ok(ValidatorResult::Ok) => {
                msg_item!(
                    "OK".green(),
                    "测试点 {} 输入合法",
                    data_item.id().to_string().bold()
                );
            }
            Ok(ValidatorResult::Invalid(message)) => {
                msg_item!(
                    "FAIL".red().bold(),
                    "测试点 {} 输入不合法",
                    data_item.id().to_string().bold()
                );
                if !message.is_empty() {
                    msg_error!("{}", message);
                }
                failed += 1;
            }
            Err(e) => {
                msg_item!(
                    "FAIL".red().bold(),
                    "测试点 {} Validator 执行失败：{}",
                    data_item.id().to_string().bold(),
                    e
                );
                failed += 1;
            }
        }

        case_pb.set_message(format!("校验测试点：{}/{}", idx + 1, data_items.len()));
        case_pb.inc(1);
    }

    if in_problem {
        case_pb.finish_with_message("校验完成！");
    } else {
        case_pb.finish_and_clear();
    }

    if failed > 0 {
        msg_error!(
            "题目 {} 有 {} 个输入未通过校验",
            problem_config.name.magenta(),
            failed
        );
    }

    Ok(())
}

async fn validate_day(
    day_config: &ContestDayConfig,
    target: Target,
    object: &str,
    in_day: bool,
) -> Result<()> {
    let total_problems = day_config.subconfig.len();
    let day_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(total_problems as u64));
    day_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.green/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    for (idx, (_, problem_config)) in day_config.subconfig.iter().enumerate() {
        day_pb.set_message(format!("处理第 {}/{} 题", idx + 1, total_problems));
        validate_problem(problem_config, target, object, false).await?;
        day_pb.inc(1);
    }
    if in_day {
        day_pb.finish_with_message("校验完成！");
    } else {
        day_pb.finish_and_clear();
    }
    Ok(())
}

pub async fn main(args: ValidateArgs) -> Result<()> {
    let Config {
        config,
        location: current_location,
    } = gctx().config.as_ref().context("找不到配置文件")?;

    match current_location {
        CurrentLocation::Problem(day_key, prob_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置：{}", day_key))?;
            let problem_config = day_config
                .subconfig
                .get(prob_key)
                .with_context(|| format!("未找到题目配置：{}", prob_key))?;
            validate_problem(problem_config, args.target, &args.object, true).await?;
        }
        CurrentLocation::Day(day_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置：{}", day_key))?;
            validate_day(day_config, args.target, &args.object, true).await?;
        }
        CurrentLocation::Root => {
            let total_days = config.subconfig.len();
            let day_pb = gctx()
                .multiprogress
                .add(ProgressBar::new(total_days as u64));
            day_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.green/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );
            for (day_idx, (_, day_config)) in config.subconfig.iter().enumerate() {
                day_pb.set_message(format!("处理第 {}/{} 天", day_idx + 1, total_days));
                validate_day(day_config, args.target, &args.object, false).await?;
                day_pb.inc(1);
            }
            day_pb.finish_with_message("校验完成！");
        }
        CurrentLocation::None => bail!("此命令必须在工程下执行"),
    }

    Ok(())
}

impl From<crate::dmk::Target> for Target {
    fn from(value: crate::dmk::Target) -> Self {
        match value {
            crate::dmk::Target::Data => Target::Data,
            crate::dmk::Target::Sample => Target::Sample,
        }
    }
}
