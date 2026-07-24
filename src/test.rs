use std::str::FromStr;
use std::time::Duration;

use bytesize::ByteSize;
use clap::{Args, ValueEnum};
use colored::Colorize;
use csv::Writer;
use evalexpr::eval_boolean;
use indicatif::ProgressBar;

use crate::config::ExpandedDataItem;
use crate::config::ScorePolicy;
use crate::config::SubtaskItem;
use crate::prelude::*;
use crate::tuack_lib::utils::testlib::Checker;
use crate::tuack_lib::utils::testlib::JudgeResult;
use crate::utils::checkers::{cpp::CppChecker, prebuilt::PrebuiltChecker};
use crate::utils::compilers::cpp::CppRunner;
use crate::utils::compilers::general::*;
use crate::utils::duration::format_duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Target {
    /// 正式测试数据
    Data,
    /// 样例数据
    Sample,
}

// *注意*：这*不是*用于测试这个程序的测试用例的命令
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ProblemStatus {
    Waiting,
    Compiling,
    CE,
    Compiled,
    Running,
}

#[derive(Debug, PartialEq, Copy, Clone)]
#[allow(unused)]
#[allow(clippy::upper_case_acronyms)]
pub enum TestCaseStatus {
    Running,
    RE,
    TLE,
    MLE,
    WA,
    AC,
    UKE,
    CE,
    PC(f64),
}

// 记录测试用例结果
#[derive(Debug)]
pub struct IndividualTestCaseResult {
    pub test_case_id: u32,
    pub status: TestCaseStatus,
    pub score: u32,
    pub max_score: u32,
    pub time: String,
    pub memory: String,
}

// 记录题目测试结果
#[derive(Debug)]
pub struct ProblemTestResult {
    pub tester_name: String,
    pub test_case_results: Vec<IndividualTestCaseResult>,
    pub total_score: u32,
    pub max_possible_score: u32,
}

#[derive(Args, Debug)]
#[command(version)]
pub struct TestArgs {
    /// 目标类型
    #[arg(value_enum, default_value = "data")]
    pub target: Target,
}

async fn run_test_case(
    runner: &mut Box<dyn Runner>,
    problem_config: &ProblemConfig,
    case: &ExpandedDataItem,
    data_dir: &str,
    checker: Option<&dyn Checker>,
) -> Result<(TestCaseStatus, Option<Duration>, Option<ByteSize>)> {
    let problem_name = &problem_config.name;
    let file_io = problem_config.file_io.unwrap_or(true);
    let input_path = problem_config.path.join(data_dir).join(&case.input);

    let input_bytes = tokio::fs::read(&input_path).await?;
    runner.set_input(input_bytes);

    if file_io {
        runner.set_io_mode(IoMode::File {
            input_name: format!("{}.in", problem_name),
            output_name: format!("{}.out", problem_name),
        });
    } else {
        runner.set_io_mode(IoMode::Stdio);
    }

    runner.set_limits(ResourceLimits::new(
        Duration::from_secs_f64(problem_config.time_limit),
        problem_config.memory_limit.as_u64(),
    ));

    let run_result = runner.execute().await?;

    let case_status = match run_result.status {
        RunStatus::Success => {
            validate_output(&run_result.output, problem_config, case, data_dir, checker)?
        }
        RunStatus::NonZeroExit(_) => TestCaseStatus::RE,
        RunStatus::TimeLimitExceeded => TestCaseStatus::TLE,
        RunStatus::MemoryLimitExceeded => TestCaseStatus::MLE,
        RunStatus::InternalError(_) => TestCaseStatus::UKE,
    };

    Ok((
        case_status,
        run_result.time,
        run_result.memory.map(ByteSize),
    ))
}

fn validate_output(
    output: &[u8],
    problem_config: &ProblemConfig,
    case: &ExpandedDataItem,
    data_dir: &str,
    checker: Option<&dyn Checker>,
) -> Result<TestCaseStatus> {
    let input_path = problem_config.path.join(data_dir).join(&case.input);
    let answer_path = problem_config.path.join(data_dir).join(&case.output);

    if output.is_empty() {
        return Ok(TestCaseStatus::WA);
    }

    let result = match checker {
        Some(chk) => chk.validate(&input_path, output, &answer_path),
        None => {
            let default_binary = gctx()
                .assets_dirs
                .iter()
                .find_map(|dir| {
                    dir.join("checkers")
                        .join("normal")
                        .exists()
                        .then(|| dir.join("checkers").join("normal"))
                })
                .unwrap_or_else(|| gctx().assets_dirs[0].join("checkers").join("normal"));
            let pchk = PrebuiltChecker::new(default_binary);
            pchk.validate(&input_path, output, &answer_path)
        }
    };

    let (judge_result, message) = match result {
        Ok(value) => value,
        Err(e) => {
            msg_warn!("Checker 执行失败：{}", e);
            return Ok(TestCaseStatus::UKE);
        }
    };

    info!("测试点信息：{}", message.trim());

    match judge_result {
        JudgeResult::Accepted => Ok(TestCaseStatus::AC),
        JudgeResult::WrongAnswer => Ok(TestCaseStatus::WA),
        JudgeResult::PresentationError => Ok(TestCaseStatus::WA),
        JudgeResult::Fail => {
            msg_warn!("SPJ 执行失败，请检查 SPJ、标程和输入输出");
            Ok(TestCaseStatus::UKE)
        }
        JudgeResult::Score(score) => Ok(TestCaseStatus::PC(score)),
    }
}

fn check_test_case(test_case: &TestCase, actual_score: u32) -> bool {
    let conditions = match &test_case.expected {
        ExpectedScore::Single(cond) => vec![cond.clone()],
        ExpectedScore::Multiple(conds) => conds.clone(),
    };

    for condition in &conditions {
        let expr = format!("{} {}", actual_score, condition);

        debug!("条件：{}", expr);

        if !eval_boolean(&expr).unwrap_or(false) {
            return false;
        }
    }

    true
}

// 将测试结果写入 CSV
fn write_results_to_csv(results: Vec<ProblemTestResult>, csv_path: &Path) -> Result<()> {
    let mut wtr = Writer::from_path(csv_path)?;

    wtr.write_record([
        "测试者",
        "测试点 ID",
        "状态",
        "得分",
        "最高分",
        "时间",
        "空间",
    ])?;

    // 写入所有测试者的结果
    for result in &results {
        // 写入每个测试用例的结果
        for test_case_result in &result.test_case_results {
            wtr.write_record(&[
                result.tester_name.clone(),
                test_case_result.test_case_id.to_string(),
                format!("{:?}", test_case_result.status),
                test_case_result.score.to_string(),
                test_case_result.max_score.to_string(),
                test_case_result.time.clone(),
                test_case_result.memory.clone(),
            ])?;
        }

        // 给这个测试者写入总分
        wtr.write_record(&[
            result.tester_name.clone(),
            "".to_string(),                        // 测试点 ID
            "TOTAL".to_string(),                   // 状态
            result.total_score.to_string(),        // 得分
            result.max_possible_score.to_string(), // 最高分
            "".to_string(),
            "".to_string(),
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

pub async fn test_problem(
    day_config: &ContestDayConfig,
    problem_config: &ProblemConfig,
    target: Target,
    in_problem: bool,
) -> Result<()> {
    let data_dir = match target {
        Target::Data => "data",
        Target::Sample => "sample",
    };

    let data_items: Vec<ExpandedDataItem> = match target {
        Target::Data => problem_config.data.clone(),
        Target::Sample => problem_config
            .samples
            .iter()
            .map(|item| ExpandedDataItem {
                id: item.id,
                score: 1,
                subtask: 0,
                input: item.input_path(),
                output: item.output_path(),
                orig_args: item.orig_args.clone(),
                args: item.args.clone(),
                dmk: item.dmk.unwrap_or(problem_config.dmk),
            })
            .collect(),
    };

    let is_sample = matches!(target, Target::Sample);

    // 准备 Checker
    let checker: Option<Box<dyn Checker>> = match &problem_config.checker {
        Some(pair) => {
            let chk_config = if is_sample {
                pair.sample.as_ref().unwrap_or(&pair.data)
            } else {
                &pair.data
            };

            let resolve = |path: &str| problem_config.path.join(path);
            let source_path = resolve(&chk_config.source);

            if !source_path.exists() {
                msg_warn!("题目 {} 的 Checker 不存在", problem_config.name.magenta());
                return Ok(());
            }

            let mut deps: HashMap<String, Vec<u8>> = HashMap::new();
            for dep_path in &chk_config.deps {
                let abs = resolve(dep_path);
                let content = match fs::read(&abs) {
                    Ok(c) => c,
                    Err(e) => {
                        msg_warn!(
                            "题目 {} 的 Checker 依赖读取失败：{}",
                            problem_config.name.magenta(),
                            e
                        );
                        return Ok(());
                    }
                };
                let name = dep_path
                    .split('/')
                    .next_back()
                    .unwrap_or(dep_path)
                    .to_string();
                deps.insert(name, content);
            }

            let compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
            compile_pb.enable_steady_tick(Duration::from_millis(100));
            compile_pb.set_message(format!("编译 {} 题目的 Checker", problem_config.name));

            let mut cpp_checker = match CppChecker::new(&source_path, &HashMap::new(), "chk", deps)
            {
                Ok(c) => c,
                Err(e) => {
                    msg_warn!(
                        "题目 {} 的 Checker 初始化失败：{}",
                        problem_config.name.magenta(),
                        e
                    );
                    return Ok(());
                }
            };

            if let Err(e) = cpp_checker.prepare() {
                msg_warn!("题目 {} 的 Checker 编译失败", problem_config.name.magenta());
                msg_warn!("{}", e);
                compile_pb.finish_and_clear();
                return Ok(());
            }

            compile_pb.finish_and_clear();
            Some(Box::new(cpp_checker) as Box<dyn Checker>)
        }
        None => None,
    };

    // 测试进度条
    let test_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(data_items.len() as u64));
    test_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.magenta/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut all_test_results = Vec::new();

    let tester_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(problem_config.tests.len() as u64));
    tester_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.yellow/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut tester_count = 0;
    for (test_name, test) in &problem_config.tests {
        tester_count += 1;
        tester_pb.set_message(format!(
            "处理第 {}/{} 个测试者：{}",
            tester_count,
            problem_config.tests.len(),
            test_name
        ));

        info!("测试 {} 的程序", test_name);
        msg_progress!(
            "测试题目 {} 的测试 {} 的程序",
            problem_config.name.magenta(),
            test_name.cyan()
        );

        let path = if PathBuf::from_str(&test.path)?.is_absolute() {
            PathBuf::from_str(&test.path)?
        } else {
            dunce::canonicalize(problem_config.path.join(&test.path))?
        };
        info!("文件路径：{}", path.display());

        let mut problem_status;

        #[allow(unused)]
        {
            problem_status = ProblemStatus::Waiting;
        }

        let mut runner: Box<dyn Runner> = match path
            .extension()
            .context("文件无后缀名")?
            .to_string_lossy()
            .to_string()
            .as_str()
        {
            "cpp" => Box::new(CppRunner::new(
                &path,
                &day_config.compile,
                problem_config.name.clone(),
            )?),
            _ => Box::new(GeneralRunner::new(
                &path,
                &day_config.compile,
                problem_config.name.clone(),
            )?),
        };

        #[allow(unused)]
        {
            problem_status = ProblemStatus::Compiling;
        }
        let res = {
            if problem_config.problem_type == ProblemType::Interactive {
                if runner.manifest().interactive {
                    let interactive = problem_config.interactive.as_ref().unwrap();

                    let resolve_path = |path: &String| -> Result<PathBuf> {
                        let p = PathBuf::from_str(path)?;
                        Ok(if p.is_absolute() {
                            p
                        } else {
                            dunce::canonicalize(problem_config.path.join(p))?
                        })
                    };

                    let grader_path = if is_sample {
                        match &interactive.sample_grader {
                            Some(sg) => resolve_path(sg)?,
                            None => resolve_path(&interactive.grader)?,
                        }
                    } else {
                        resolve_path(&interactive.grader)?
                    };
                    let header_path = resolve_path(&interactive.header)?;

                    if !grader_path.exists() {
                        bail!("grader 不存在")
                    }
                    if !header_path.exists() {
                        bail!("header 不存在")
                    }

                    runner.set_interactive(&grader_path, &header_path)?;
                } else {
                    bail!("该语言不支持交互")
                }
            }
            runner.prepare()
        };
        match res {
            Ok(_) => {
                problem_status = ProblemStatus::Compiled;
            }
            Err(e) => {
                #[allow(unused)]
                {
                    problem_status = ProblemStatus::CE;
                }
                msg_item!("CE".yellow().bold(), "编译错误");
                msg_error!("{}", e);
            }
        };

        let mut total_score: u32 = 0;

        let mut subtask_scores: HashMap<u32, Vec<u32>> = if is_sample {
            HashMap::from([(0, vec![])])
        } else {
            problem_config
                .subtasks
                .keys()
                .map(|id| (*id, vec![]))
                .collect()
        };

        if problem_status == ProblemStatus::Compiled {
            #[allow(unused)]
            {
                problem_status = ProblemStatus::Running;
            }
            let mut individual_results = Vec::new();
            let mut case_count = 0;

            let case_test_pb = gctx()
                .multiprogress
                .add(ProgressBar::new(data_items.len() as u64));
            case_test_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.magenta/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

            case_test_pb.set_message(format!("运行测试点：{}/{}", 1, data_items.len()));

            for case in &data_items {
                case_count += 1;
                info!("运行测试点：{}", case.id);

                let run_result = run_test_case(
                    &mut runner,
                    problem_config,
                    case,
                    data_dir,
                    checker.as_deref(),
                )
                .await?;
                let case_status = run_result.0;
                info!("测试点结果：{:?}", case_status);

                let earned_score = match case_status {
                    TestCaseStatus::AC => case.score,
                    TestCaseStatus::PC(partial) => {
                        ((partial / 100.0) * (case.score as f64)).round() as u32
                    }
                    _ => 0,
                };
                subtask_scores
                    .get_mut(&case.subtask)
                    .context("不存在指定的 Subtask")?
                    .push(earned_score);

                individual_results.push(IndividualTestCaseResult {
                    test_case_id: case.id,
                    status: case_status,
                    score: earned_score,
                    max_score: case.score,
                    time: match run_result.1 {
                        Some(duration) => format_duration(duration),
                        None => "N/A".to_string(),
                    },
                    memory: match run_result.2 {
                        Some(memory) => format!("{}", memory),
                        None => "N/A".to_string(),
                    },
                });

                let status_str = match case_status {
                    TestCaseStatus::AC => "AC".green(),
                    TestCaseStatus::WA => "WA".red(),
                    TestCaseStatus::TLE => "TLE".blue(),
                    TestCaseStatus::MLE => "MLE".blue(),
                    TestCaseStatus::RE => "RE".bright_blue(),
                    TestCaseStatus::UKE => "UKE".bright_black(),
                    TestCaseStatus::Running => unreachable!(),
                    TestCaseStatus::CE => "CE".yellow(),
                    TestCaseStatus::PC(score) => format!("PC {:.2} / 100", score).yellow(),
                };
                msg_item!(
                    status_str.clone().bold(),
                    "测试点 {}  | {} | {}",
                    case.id.to_string().bold(),
                    match run_result.1 {
                        Some(duration) => format_duration(duration),
                        None => "N/A".to_string(),
                    }
                    .bold(),
                    match run_result.2 {
                        Some(memory) => format!("{}", memory),
                        None => "N/A".to_string(),
                    }
                    .bold()
                );

                case_test_pb.set_message(format!(
                    "运行测试点：{}/{} | #{} {}",
                    case_count + 1,
                    data_items.len(),
                    case.id,
                    status_str
                ));
                case_test_pb.inc(1);
            }

            case_test_pb.finish_and_clear();

            msg_info!("测试结果：");
            let scoring_subtasks: BTreeMap<u32, SubtaskItem> = if is_sample {
                BTreeMap::from([(
                    0,
                    SubtaskItem {
                        items: vec![],
                        max_score: data_items.len() as u32,
                        policy: ScorePolicy::Sum,
                    },
                )])
            } else {
                problem_config.subtasks.clone()
            };
            for (id, subtask) in &scoring_subtasks {
                let scores = &subtask_scores[id];
                let subtask_score = match subtask.policy {
                    ScorePolicy::Sum => scores.iter().sum(),
                    ScorePolicy::Max => *scores.iter().max().unwrap_or(&0),
                    ScorePolicy::Min => *scores.iter().min().unwrap_or(&0),
                };
                info!(
                    "Subtask #{} 得分 {}/{}",
                    id, subtask_score, subtask.max_score
                );
                msg_info!(
                    "Subtask {}{} 得分 {}/{}",
                    "#".bold(),
                    id.to_string().bold(),
                    subtask_score.to_string().cyan(),
                    subtask.max_score.to_string().green()
                );
                total_score += subtask_score;
            }

            let problem_result = ProblemTestResult {
                tester_name: test_name.to_string(),
                test_case_results: individual_results,
                total_score,
                max_possible_score: scoring_subtasks.iter().map(|task| task.1.max_score).sum(),
            };
            msg_info!(
                "总得分 {}/{}",
                total_score.to_string().cyan().bold(),
                problem_result.max_possible_score.to_string().green().bold()
            );
            all_test_results.push(problem_result);
        } else {
            let scoring_subtasks: BTreeMap<u32, SubtaskItem> = if is_sample {
                BTreeMap::from([(
                    0,
                    SubtaskItem {
                        items: vec![],
                        max_score: data_items.len() as u32,
                        policy: ScorePolicy::Sum,
                    },
                )])
            } else {
                problem_config.subtasks.clone()
            };
            let max_possible: u32 = scoring_subtasks.iter().map(|task| task.1.max_score).sum();
            let problem_result = ProblemTestResult {
                tester_name: test_name.to_string(),
                test_case_results: vec![IndividualTestCaseResult {
                    test_case_id: 0,
                    status: TestCaseStatus::CE,
                    score: 0,
                    max_score: data_items.iter().map(|case| case.score).sum(),
                    time: "N/A".to_string(),
                    memory: "N/A".to_string(),
                }],
                total_score: 0,
                max_possible_score: max_possible,
            };
            msg_info!(
                "总得分 {}/{}",
                0.to_string().cyan().bold(),
                problem_result.max_possible_score.to_string().green().bold()
            );
            all_test_results.push(problem_result);
        }

        info!(
            "总得分：{}/{}",
            total_score,
            data_items.iter().map(|case| case.score).sum::<u32>()
        );

        if target == Target::Data {
            if check_test_case(test, total_score) {
                info!("测试 {} 通过", test_name);
            } else {
                info!("测试 {} 不满足所有条件", test_name);
                msg_warn!("{}", "不满足所有条件".bold());
            }
        }

        tester_pb.inc(1);
        runner.cleanup()?;
    }

    if in_problem {
        tester_pb.finish_with_message("测试完成！");
    } else {
        tester_pb.finish_and_clear();
    }

    let csv_path = problem_config.path.join(match target {
        Target::Data => "result.csv",
        Target::Sample => "result-sample.csv",
    });
    write_results_to_csv(all_test_results, &csv_path)?;
    test_pb.finish_and_clear();

    Ok(())
}

async fn test_day(day_config: &ContestDayConfig, target: Target, in_day: bool) -> Result<()> {
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
        test_problem(day_config, problem_config, target, false).await?;
        day_pb.inc(1);
    }
    if in_day {
        day_pb.finish_with_message("测试完成！");
    } else {
        day_pb.finish_and_clear();
    }
    Ok(())
}

pub async fn main(args: TestArgs) -> Result<()> {
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
            test_problem(day_config, problem_config, args.target, true).await?;
        }
        CurrentLocation::Day(day_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置：{}", day_key))?;
            test_day(day_config, args.target, true).await?;
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
                test_day(day_config, args.target, false).await?; // 复用 test_day
                day_pb.inc(1);
            }
            day_pb.finish_with_message("测试完成！");
        }
        CurrentLocation::None => bail!("此命令必须在工程下执行"),
    }

    Ok(())
}
