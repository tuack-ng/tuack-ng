use crate::config::ScorePolicy;
use crate::config::lang::Language;
use crate::prelude::*;
use crate::test::checker::parse_result;
use crate::utils::compile::build_compile_cmd;
use bytesize::ByteSize;
use clap::Args;
use colored::Colorize;
use csv::Writer;
use evalexpr::eval_boolean;
use indicatif::ProgressBar;
use shared_child::SharedChild;
use std::{
    process::{Command, Stdio},
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

pub mod checker;

// **注意**：这**不是**用于测试这个程序的测试用例的命令
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

fn create_or_clear_dir(path: &Path) -> Result<(), std::io::Error> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    fs::create_dir_all(path)
}

fn run_test_case(
    program_path: &Path,
    problem_name: &String,
    input_path: &Path,
    time_limit_ms: u128,
    memory_limit_bytes: u64,
    file_io: bool,
) -> Result<TestCaseStatus> {
    // 复制输入文件
    let program_dir = program_path.parent().unwrap();
    let test_input_path = if file_io {
        program_dir.join(format!("{}.in", problem_name))
    } else {
        program_dir.join(format!("{}.stdin", problem_name))
    };
    fs::copy(input_path, &test_input_path)?;

    // 启动程序
    let child = if file_io {
        SharedChild::spawn(
            Command::new(program_path)
                .current_dir(program_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )?
    } else {
        let stdin_file = fs::File::open(&test_input_path)?;
        let stdout_file = fs::File::create(program_dir.join(format!("{}.stdout", problem_name)))?;
        SharedChild::spawn(
            Command::new(program_path)
                .current_dir(program_dir)
                .stdin(Stdio::from(stdin_file))
                .stdout(Stdio::from(stdout_file))
                .stderr(Stdio::piped()),
        )?
    };

    let child = Arc::new(child);
    let child_clone = child.clone();
    let start = Instant::now();

    // 用于监控线程和主线程通信的状态
    let case_status = Arc::new(Mutex::new(TestCaseStatus::Running));
    let status_clone = case_status.clone();

    // 启动监控线程
    let monitor_thread = thread::spawn(move || {
        let mut sys = System::new();
        let pid = Pid::from_u32(child_clone.id());

        loop {
            #[cfg(windows)] // or maybe TLE
            thread::sleep(Duration::from_millis(100));
            #[cfg(not(windows))]
            thread::sleep(Duration::from_millis(50));
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);

            // 检查时间限制
            if start.elapsed().as_millis() > time_limit_ms {
                let _ = child_clone.kill();
                *status_clone.lock().unwrap() = TestCaseStatus::TLE;
                break;
            }

            // 检查内存限制
            if let Some(process) = sys.process(pid)
                && process.memory() > memory_limit_bytes
            {
                let _ = child_clone.kill();
                *status_clone.lock().unwrap() = TestCaseStatus::MLE;
                break;
            }

            // 检查进程是否已退出
            if sys.process(pid).is_none() {
                break;
            }
        }
    });

    // 等待程序结束
    let run_status = child.wait()?;
    monitor_thread.join().unwrap();

    // 获取最终状态
    let mut final_status = *case_status.lock().unwrap();

    // 如果监控线程没有标记超限，但程序运行失败，则标记为RE
    if final_status == TestCaseStatus::Running && !run_status.success() {
        final_status = TestCaseStatus::RE;
    }

    Ok(final_status)
}

fn validate_output(
    program_dir: &Path,
    problem_name: &str,
    answer_path: &Path,
    file_io: bool,
    spj: Option<PathBuf>,
) -> Result<TestCaseStatus> {
    let output_path = if file_io {
        program_dir.join(format!("{}.out", problem_name))
    } else {
        program_dir.join(format!("{}.stdout", problem_name))
    };
    let input_path = if file_io {
        program_dir.join(format!("{}.in", problem_name))
    } else {
        program_dir.join(format!("{}.stdin", problem_name))
    };

    // 检查输出文件是否存在
    if !output_path.exists() {
        return Ok(TestCaseStatus::WA);
    }

    // 复制答案文件
    let ans_path = program_dir.join(format!("{}.ans", problem_name));
    fs::copy(answer_path, &ans_path)?;

    let checker_path = match spj {
        Some(spj_path) => spj_path,
        // 查找第一个存在的 checker 文件
        None => get_context()
            .assets_dirs
            .iter()
            .find_map(|dir| {
                dir.join("checkers")
                    .join("normal")
                    .exists()
                    .then(|| dir.join("checkers").join("normal"))
            })
            .unwrap_or_else(|| get_context().assets_dirs[0].join("checkers").join("normal")),
    };

    let res_path = program_dir.join(format!("{}.res", problem_name));

    // 使用校验器验证
    let _validate = Command::new(&checker_path)
        .arg(&input_path)
        .arg(&output_path)
        .arg(&ans_path)
        .arg(&res_path)
        .arg("-appes")
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()?;

    let res_content = match fs::read_to_string(res_path) {
        Ok(content) => content,
        Err(e) => {
            warn!("无法读取校验器结果文件: {}", e);
            return Ok(TestCaseStatus::UKE);
        }
    };

    let res = parse_result(&res_content)?;

    info!("测试点信息: {}", res.1.trim());

    match res.0 {
        checker::JudgeResult::Accepted => Ok(TestCaseStatus::AC),
        checker::JudgeResult::WrongAnswer => Ok(TestCaseStatus::WA),
        checker::JudgeResult::PresentationError => Ok(TestCaseStatus::WA),
        checker::JudgeResult::Fail => {
            warn!("SPJ 执行失败，请检查 SPJ、标程和输入输出");
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

    wtr.write_record(["测试者", "测试点ID", "状态", "得分", "最高分"])?;

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
            ])?;
        }

        // 给这个测试者写入总分
        wtr.write_record(&[
            result.tester_name.clone(),
            "".to_string(),                        // 测试点ID
            "TOTAL".to_string(),                   // 状态
            result.total_score.to_string(),        // 得分
            result.max_possible_score.to_string(), // 最高分
        ])?;
    }

    wtr.flush()?;
    Ok(())
}

fn check_compiler(language: &Language) -> Result<()> {
    // 检查编译环境
    let compiler = &language.compiler;
    debug!("检查 {} 环境", language.language);
    match Command::new(&compiler.executable)
        .arg(&compiler.version_check)
        .output()
    {
        Ok(output) if output.status.success() => {
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version = version_output.lines().next().unwrap_or("").trim();
            debug!("{} 版本: {}", &compiler.executable, version);
        }
        _ => {
            error!(
                "未找到 {} 命令，请确保已安装并添加到PATH",
                &compiler.executable
            );
            bail!("{} 命令执行失败", &compiler.executable);
        }
    }
    Ok(())
}

pub fn main(_: TestArgs) -> Result<()> {
    let (config, current_location) = get_context().config.as_ref().context("找不到配置文件")?;

    let (skip_level, target_day_key, target_problem_key) = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => {
            (2, Some(day_name.as_str()), Some(problem_name.as_str()))
        }
        CurrentLocation::Day(day_name) => (1, Some(day_name.as_str()), None),
        _ => (0, None, None),
    };

    // 获取要处理的天配置
    let days_vec;
    let days_to_process: Vec<(&String, &crate::config::ContestDayConfig)> =
        if let Some(day_key) = target_day_key {
            let day_config = config
                .subconfig
                .get(day_key)
                .with_context(|| format!("未找到天配置: {}", day_key))?;
            let actual_key = config
                .subconfig
                .keys()
                .find(|k| k.as_str() == day_key)
                .with_context(|| format!("未找到天配置键: {}", day_key))?;
            days_vec = vec![(actual_key, day_config)];
            days_vec.iter().map(|(k, v)| (*k, *v)).collect()
        } else {
            config.subconfig.iter().collect()
        };

    let total_days = days_to_process.len();

    let day_pb = if skip_level >= 1 {
        get_context().multiprogress.add(ProgressBar::new(0))
    } else {
        let pb = get_context()
            .multiprogress
            .add(ProgressBar::new(total_days as u64));
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.green/blue}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
        pb
    };

    let mut day_count = 0;
    for (_day_key, day_config) in days_to_process {
        day_count += 1;
        if skip_level < 1 {
            day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
        }
        info!("处理天: {}", day_config.name);

        // 获取要处理的问题
        let problems_vec;
        let problems_to_process: Vec<(&String, &crate::config::ProblemConfig)> =
            if let Some(problem_key) = target_problem_key {
                let problem_config = day_config
                    .subconfig
                    .get(problem_key)
                    .with_context(|| format!("未找到问题: {}", problem_key))?;
                let actual_key = day_config
                    .subconfig
                    .keys()
                    .find(|k| k.as_str() == problem_key)
                    .with_context(|| format!("未找到问题键: {}", problem_key))?;
                problems_vec = vec![(actual_key, problem_config)];
                problems_vec.iter().map(|(k, v)| (*k, *v)).collect()
            } else {
                day_config.subconfig.iter().collect()
            };

        let problem_count_in_day = problems_to_process.len();

        let problem_pb = if skip_level >= 2 {
            get_context().multiprogress.add(ProgressBar::new(0))
        } else {
            let pb = get_context()
                .multiprogress
                .add(ProgressBar::new(problem_count_in_day as u64));
            pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.cyan/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );
            pb
        };

        let mut problem_count = 0;
        for (_problem_key, problem_config) in problems_to_process {
            problem_count += 1;
            if skip_level < 2 {
                problem_pb.set_message(format!(
                    "处理第 {}/{} 题",
                    problem_count, problem_count_in_day
                ));
            }

            info!("处理题目: {}", problem_config.name);

            if let Some(use_chk) = problem_config.use_chk
                && use_chk
            {
                info!("使用自定义 chk 设置: {}", use_chk);

                let compile_pb = get_context().multiprogress.add(ProgressBar::new_spinner());
                compile_pb.enable_steady_tick(Duration::from_millis(100));
                compile_pb.set_message(format!("编译 {} 题目的 spj", problem_config.name));

                let chk_path = problem_config.path.join("data").join("chk").join("chk.cpp");
                if !chk_path.exists() {
                    warn!("chk 文件不存在，跳过测试此题目");
                    continue;
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
                    warn!(
                        "chk 编译失败，跳过测试此题目: \n{}",
                        String::from_utf8_lossy(&compile_output.stderr)
                    );
                    continue;
                }
                compile_pb.finish_and_clear();
            }

            let test_pb = get_context()
                .multiprogress
                .add(ProgressBar::new(problem_config.data.len() as u64));
            test_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.magenta/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

            let mut all_test_results = Vec::new();

            let tester_pb = get_context()
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

                let path = if PathBuf::from_str(&test.path)?.is_absolute() {
                    PathBuf::from_str(&test.path)?
                } else {
                    dunce::canonicalize(problem_config.path.join(&test.path))?
                };

                info!("文件路径：{}", path.display());

                #[allow(unused_assignments)]
                let mut problem_status = ProblemStatus::Waiting;

                let tmp_dir = path.parent().unwrap().join("tmp");
                create_or_clear_dir(&tmp_dir)?;

                let target_path = tmp_dir.join(path.file_name().unwrap());
                fs::copy(&path, &target_path)?;

                #[allow(unused_assignments)]
                {
                    problem_status = ProblemStatus::Compiling;
                }
                info!("正在编译...");

                let ext = target_path
                    .extension()
                    .context("文件无后缀名")?
                    .to_string_lossy();

                let file_type = get_context()
                    .languages
                    .get(ext.as_ref())
                    .context("未知格式文件")?;

                check_compiler(file_type)?;

                let compile_status = build_compile_cmd(
                    day_config,
                    problem_config,
                    &target_path,
                    ext.to_string(),
                    file_type,
                )?
                .current_dir(&tmp_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;

                if compile_status.success() {
                    problem_status = ProblemStatus::Compiled;
                    info!("编译成功");
                } else {
                    problem_status = ProblemStatus::CE;
                    info!("编译错误");
                }

                let mut total_score: u32 = 0;

                fs::remove_file(&target_path)?;

                let mut subtask_scores: HashMap<u32, Vec<u32>> = problem_config
                    .subtests
                    .keys()
                    .map(|id| (*id, Vec::new()))
                    .collect();

                if problem_status == ProblemStatus::Compiled {
                    #[allow(unused_assignments)]
                    {
                        problem_status = ProblemStatus::Running;
                    }
                    let program_path = tmp_dir.join(&problem_config.name);

                    let mut individual_results = Vec::new();
                    let mut case_count = 0;

                    let case_test_pb = get_context()
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

                        let input_path = problem_config
                            .path
                            .join("data")
                            .join(case.input.get().unwrap());
                        let answer_path = problem_config
                            .path
                            .join("data")
                            .join(case.output.get().unwrap());

                        info!("运行测试点: {}", case.id);

                        let run_result = run_test_case(
                            &program_path,
                            &problem_config.name,
                            &input_path,
                            (problem_config.time_limit * 1000.0) as u128,
                            ByteSize::from_str(&problem_config.memory_limit)
                                .unwrap_or_else(|e| {
                                    warn!(
                                        "空间限制东西字符串转换失败: {}, 使用 512 MiB 作为默认值",
                                        e
                                    );
                                    ByteSize::mib(512)
                                })
                                .as_u64(),
                            problem_config.file_io.unwrap_or(true),
                        )?;

                        let case_status = match run_result {
                            TestCaseStatus::Running => validate_output(
                                &tmp_dir,
                                &problem_config.name,
                                &answer_path,
                                problem_config.file_io.unwrap_or(true),
                                if problem_config.use_chk.unwrap_or(false) {
                                    Some(problem_config.path.join("data").join("chk").join("chk"))
                                } else {
                                    None
                                },
                            )?,
                            status => status,
                        };

                        info!("测试点结果: {:?}", case_status);

                        let earned_score = match case_status {
                            TestCaseStatus::AC => case.score,
                            TestCaseStatus::PC(partial) => {
                                ((partial / 100.0) * (case.score as f64)).round() as u32
                            }
                            _ => 0,
                        };
                        subtask_scores
                            .get_mut(&case.subtest)
                            .context("不存在指定的 Subtask")?
                            .push(earned_score);

                        individual_results.push(IndividualTestCaseResult {
                            test_case_id: case.id,
                            status: case_status,
                            score: earned_score,
                            max_score: case.score,
                        });

                        let status_str = match case_status {
                            TestCaseStatus::AC => "AC".green(),
                            TestCaseStatus::WA => "WA".red(),
                            TestCaseStatus::TLE => "TLE".blue(),
                            TestCaseStatus::MLE => "MLE".blue(),
                            TestCaseStatus::RE => "RE".bright_blue(),
                            TestCaseStatus::UKE => "UKE".bright_black(),
                            TestCaseStatus::Running => "Running".yellow(),
                            TestCaseStatus::CE => "CE".yellow(),
                            TestCaseStatus::PC(score) => format!("PC {:.2} / 100", score).yellow(),
                        };
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

                    for (id, policy) in &problem_config.subtests {
                        let scores = &subtask_scores[id];

                        let subtest_score = match policy {
                            ScorePolicy::Sum => scores.iter().sum(),
                            ScorePolicy::Max => *scores.iter().max().unwrap_or(&0),
                            ScorePolicy::Min => *scores.iter().min().unwrap_or(&0),
                        };

                        info!("Subtask #{} 得分 {}", id, subtest_score);

                        total_score += subtest_score;
                    }

                    let problem_result = ProblemTestResult {
                        tester_name: test_name.to_string(),
                        test_case_results: individual_results,
                        total_score,
                        max_possible_score: problem_config.data.iter().map(|case| case.score).sum(),
                    };

                    all_test_results.push(problem_result);
                } else {
                    let problem_result = ProblemTestResult {
                        tester_name: test_name.to_string(),
                        test_case_results: vec![IndividualTestCaseResult {
                            test_case_id: 0,
                            status: TestCaseStatus::CE,
                            score: 0,
                            max_score: problem_config.data.iter().map(|case| case.score).sum(),
                        }],
                        total_score: 0,
                        max_possible_score: problem_config.data.iter().map(|case| case.score).sum(),
                    };
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
                    warn!("测试 {} 不满足所有条件", test_name);
                }

                tester_pb.inc(1);

                let _ = fs::remove_dir_all(&tmp_dir);
            }

            tester_pb.finish_and_clear();

            write_results_to_csv(all_test_results, &problem_config.path)?;

            if skip_level == 2 {
                test_pb.finish_with_message("测试完成！");
            } else {
                test_pb.finish_and_clear();
            }

            if skip_level >= 2 {
                break;
            }
        }

        if skip_level == 1 {
            problem_pb.finish_with_message("测试完成！");
        } else {
            problem_pb.finish_and_clear();
        }

        if skip_level >= 1 {
            break;
        }
    }

    if skip_level == 0 {
        day_pb.finish_with_message("测试完成！");
    } else {
        info!("测试完成！");
    }

    Ok(())
}
