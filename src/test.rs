use crate::prelude::*;
use crate::test::checker::parse_result;
use crate::tuack_lib::config::ExpandedDataItem;
use crate::tuack_lib::config::ScorePolicy;
use crate::utils::compiler::*;
use crate::utils::duration::format_duration;
use bytesize::ByteSize;
use clap::Args;
use colored::Colorize;
use csv::Writer;
use evalexpr::eval_boolean;
use indicatif::ProgressBar;
use std::cmp::max;
use std::sync::{Arc, Mutex};
use std::{
    process::{Command, Stdio},
    str::FromStr,
    time::Duration,
};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tempfile::NamedTempFile;
pub mod checker;

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
pub struct TestArgs {}

async fn run_test_case(
    runner: &mut GeneralRunner,
    problem_config: &ProblemConfig,
    case: &Arc<ExpandedDataItem>,
) -> Result<(TestCaseStatus, Option<Duration>, Option<ByteSize>)> {
    let problem_name = &problem_config.name;
    let time_limit_ms = (problem_config.time_limit * 1000.0) as u128;
    let memory_limit_bytes = problem_config.memory_limit.as_u64();
    let file_io = problem_config.file_io.unwrap_or(true);
    let input_path = problem_config.path.join("data").join(&case.input);

    use tokio::time::{Duration, Instant, sleep};

    if file_io {
        runner.set_file_io(
            &input_path,
            &format!("{}.in", problem_name),
            &format!("{}.out", problem_name),
        )?;
    } else {
        runner.set_std_io(&input_path)?;
    }

    let result = async {
        let mut child = runner.get_run_async().await?.spawn()?;

        let pid = child.id().unwrap() as u32;
        let start = Instant::now();
        let time_limit = Duration::from_millis(time_limit_ms as u64);

        // 使用 Arc 和 Mutex 来在线程间共享内存使用数据
        let peak_memory = Arc::new(Mutex::new(0u64));
        let monitoring_peak_memory = Arc::clone(&peak_memory);
        let result = tokio::select! {
            biased;

            // 内存和超时监控任务
            _ = async move {
                let mut sys = System::new();
                let sys_pid = Pid::from_u32(pid);

                loop {
                    // 定期检查
                    sleep(Duration::from_millis(10)).await;

                    // 检查超时
                    if start.elapsed() > time_limit+Duration::from_millis(200) {
                        return;
                    }

                    // 检查内存
                    sys.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), false);
                    if let Some(process) = sys.process(sys_pid) {
                        let memory = process.memory();
                        let mut peak = monitoring_peak_memory.lock().unwrap();

                        *peak = max(*peak,memory);
                        if memory > memory_limit_bytes {
                            return;
                        }
                    } else {
                        // 进程已结束
                        return;
                    }
                }
            } => {
                // 内存超限或超时
                let _ = child.kill().await;

                // 获取记录的峰值内存
                let final_peak = *peak_memory.lock().unwrap();

                // 需要区分是超时还是MLE
                if start.elapsed() > time_limit {
                    info!("测试点超时");
                    (TestCaseStatus::TLE,None,None)
                } else {
                    info!("测试点内存超限，峰值内存: {} bytes", ByteSize(final_peak));
                    (TestCaseStatus::MLE,None,None)
                }
            }

            // 等待进程结束
            exit_status = child.wait() => {
                let elapsed_time = start.elapsed();

                // 获取记录的峰值内存
                let final_peak = *peak_memory.lock().unwrap();

                info!("测试点运行完成，使用时间: {:?}, 峰值内存: {}", elapsed_time, ByteSize(final_peak));

                match exit_status {
                    Ok(status) if status.success() => {
                        (TestCaseStatus::Running,Some(elapsed_time),Some(ByteSize(final_peak)))
                    },
                    Ok(_) => {
                        (TestCaseStatus::RE,Some(elapsed_time),Some(ByteSize(final_peak)))
                    },
                    Err(e) => {
                        msg_error!("测试点运行出现内部错误: {}", e);
                        (TestCaseStatus::UKE,None,None)
                    },
                }
            }
        };
        Ok::<(TestCaseStatus, Option<Duration>, Option<ByteSize>), anyhow::Error>(result)
    }.await?;

    let case_status = match result.0 {
        TestCaseStatus::Running => validate_output(runner, problem_config, case)?,
        status => status,
    };

    Ok((case_status, result.1, result.2))
}

fn validate_output(
    runner: &GeneralRunner,
    problem_config: &ProblemConfig,
    case: &Arc<ExpandedDataItem>,
) -> Result<TestCaseStatus> {
    let input_path = problem_config.path.join("data").join(&case.input);
    let output_path = runner.get_output_path()?;
    let answer_path = problem_config.path.join("data").join(&case.output);
    let spj = if problem_config.use_chk.unwrap_or(false) {
        Some(problem_config.path.join("data").join("chk").join("chk"))
    } else {
        None
    };

    // 检查输出文件是否存在
    if !output_path.exists() {
        return Ok(TestCaseStatus::WA);
    }

    let checker_path = match spj {
        Some(spj_path) => spj_path,
        // 查找第一个存在的 checker 文件
        None => gctx()
            .assets_dirs
            .iter()
            .find_map(|dir| {
                dir.join("checkers")
                    .join("normal")
                    .exists()
                    .then(|| dir.join("checkers").join("normal"))
            })
            .unwrap_or_else(|| gctx().assets_dirs[0].join("checkers").join("normal")),
    };

    let res_path = NamedTempFile::with_prefix("tuack-ng-test-res-")?;

    debug!("{}", checker_path.display());

    // 使用校验器验证
    let _validate = Command::new(&checker_path)
        .arg(&input_path)
        .arg(&output_path)
        .arg(&answer_path)
        .arg(&res_path.path())
        .arg("-appes")
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()?;

    let res_content = match fs::read_to_string(res_path) {
        Ok(content) => content,
        Err(e) => {
            msg_warn!("无法读取校验器结果文件: {}", e);
            return Ok(TestCaseStatus::UKE);
        }
    };

    let res = match parse_result(&res_content) {
        Ok(value) => value,
        Err(e) => {
            msg_warn!("无法解析校验器结果: {:?}", e);
            (checker::JudgeResult::Fail, "无法解析校验器结果".into())
        }
    };

    info!("测试点信息: {}", res.1.trim());

    match res.0 {
        checker::JudgeResult::Accepted => Ok(TestCaseStatus::AC),
        checker::JudgeResult::WrongAnswer => Ok(TestCaseStatus::WA),
        checker::JudgeResult::PresentationError => Ok(TestCaseStatus::WA),
        checker::JudgeResult::Fail => {
            msg_warn!("SPJ 执行失败，请检查 SPJ、标程和输入输出");
            Ok(TestCaseStatus::UKE)
        }
        checker::JudgeResult::Score(score) => Ok(TestCaseStatus::PC(score)),
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
fn write_results_to_csv(results: Vec<ProblemTestResult>, problem_path: &Path) -> Result<()> {
    let csv_path = problem_path.join("result.csv");

    let mut wtr = Writer::from_path(&csv_path)?;

    wtr.write_record([
        "测试者",
        "测试点ID",
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
            "".to_string(),                        // 测试点ID
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
) -> Result<()> {
    // 如果使用自定义 SPJ，先编译
    if let Some(use_chk) = problem_config.use_chk
        && use_chk
    {
        let compile_pb = gctx().multiprogress.add(ProgressBar::new_spinner());
        compile_pb.enable_steady_tick(Duration::from_millis(100));
        compile_pb.set_message(format!("编译 {} 题目的 spj", problem_config.name));

        let chk_path = problem_config.path.join("data").join("chk").join("chk.cpp");
        if !chk_path.exists() {
            msg_warn!("题目 {} 的 Checker 不存在", problem_config.name.magenta());
            return Ok(());
        }

        let compile_output = Command::new("g++")
            .arg("-o")
            .arg(problem_config.path.join("data").join("chk").join("chk"))
            .arg(&chk_path)
            .arg("-O2")
            .arg("-std=c++23")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()?;
        if !compile_output.status.success() {
            msg_warn!("题目 {} 的 Checker 编译失败", problem_config.name.magenta());
            msg_warn!("{}", String::from_utf8_lossy(&compile_output.stderr));
            return Ok(());
        }
        compile_pb.finish_and_clear();
    }

    // 测试进度条
    let test_pb = gctx()
        .multiprogress
        .add(ProgressBar::new(problem_config.data.len() as u64));
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
            "处理第 {}/{} 个测试者: {}",
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

        let mut runner =
            GeneralRunner::new(&path, &day_config.compile, problem_config.name.clone())?;
        #[allow(unused)]
        {
            problem_status = ProblemStatus::Compiling;
        }
        match runner.prepare() {
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

        let mut subtask_scores: HashMap<u32, Vec<u32>> = problem_config
            .subtasks
            .keys()
            .map(|id| (*id, Vec::new()))
            .collect();

        if problem_status == ProblemStatus::Compiled {
            #[allow(unused)]
            {
                problem_status = ProblemStatus::Running;
            }
            let mut individual_results = Vec::new();
            let mut case_count = 0;

            let case_test_pb = gctx()
                .multiprogress
                .add(ProgressBar::new(problem_config.data.len() as u64));
            case_test_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.magenta/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

            for case in &problem_config.data {
                case_count += 1;
                info!("运行测试点: {}", case.id);

                let run_result = run_test_case(&mut runner, problem_config, case).await?;
                let case_status = run_result.0;
                info!("测试点结果: {:?}", case_status);

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
                    "运行测试点: {}/{} | #{} {}",
                    case_count,
                    problem_config.data.len(),
                    case.id,
                    status_str
                ));
                case_test_pb.inc(1);
            }
            case_test_pb.finish_and_clear();

            msg_info!("测试结果:");
            for (id, subtask) in &problem_config.subtasks {
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
                max_possible_score: problem_config
                    .subtasks
                    .iter()
                    .map(|task| task.1.max_score)
                    .sum(),
            };
            msg_info!(
                "总得分 {}/{}",
                total_score.to_string().cyan().bold(),
                problem_result.max_possible_score.to_string().green().bold()
            );
            all_test_results.push(problem_result);
        } else {
            let problem_result = ProblemTestResult {
                tester_name: test_name.to_string(),
                test_case_results: vec![IndividualTestCaseResult {
                    test_case_id: 0,
                    status: TestCaseStatus::CE,
                    score: 0,
                    max_score: problem_config.data.iter().map(|case| case.score).sum(),
                    time: "N/A".to_string(),
                    memory: "N/A".to_string(),
                }],
                total_score: 0,
                max_possible_score: problem_config
                    .subtasks
                    .iter()
                    .map(|task| task.1.max_score)
                    .sum(),
            };
            msg_info!(
                "总得分 {}/{}",
                0.to_string().cyan().bold(),
                problem_result.max_possible_score.to_string().green().bold()
            );
            all_test_results.push(problem_result);
        }

        info!(
            "总得分: {}/{}",
            total_score,
            problem_config
                .data
                .iter()
                .map(|case| case.score)
                .sum::<u32>()
        );

        if check_test_case(test, total_score) {
            info!("测试 {} 通过", test_name);
        } else {
            info!("测试 {} 不满足所有条件", test_name);
            msg_warn!("{}", "不满足所有条件".bold());
        }

        tester_pb.inc(1);
        runner.cleanup()?;
    }

    tester_pb.finish_and_clear();
    write_results_to_csv(all_test_results, &problem_config.path)?;
    test_pb.finish_and_clear();

    Ok(())
}

async fn test_day(day_config: &ContestDayConfig) -> Result<()> {
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
        test_problem(day_config, problem_config).await?;
        day_pb.inc(1);
    }
    day_pb.finish_with_message("当天所有题目测试完成");
    Ok(())
}

pub async fn main(_: TestArgs) -> Result<()> {
    let (config, current_location) = gctx().config.as_ref().context("找不到配置文件")?;

    match current_location {
        CurrentLocation::Problem(day_key, prob_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置: {}", day_key))?;
            let problem_config = day_config
                .subconfig
                .get(prob_key)
                .with_context(|| format!("未找到题目配置: {}", prob_key))?;
            test_problem(day_config, problem_config).await?;
        }
        CurrentLocation::Day(day_key) => {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置: {}", day_key))?;
            test_day(day_config).await?;
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
                test_day(day_config).await?; // 复用 test_day
                day_pb.inc(1);
            }
            day_pb.finish_with_message("所有题目测试完成");
        }
        CurrentLocation::None => bail!("此命令必须在工程下执行"),
    }

    Ok(())
}
