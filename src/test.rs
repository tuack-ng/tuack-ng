use crate::config::lang::Language;
// **注意**：这**不是**用于测试这个程序的测试用例的命令
use crate::config::{ExpectedScore, TestCase};
use crate::context::CurrentLocation;
use crate::context::get_context;
use bytesize::ByteSize;
use clap::Args;
use colored::Colorize;
use csv::Writer;
use evalexpr::{ContextWithMutableVariables, HashMapContext, Value, eval_boolean_with_context_mut};
use indicatif::ProgressBar;
use log::{debug, error, info, warn};
use shared_child::SharedChild;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ProblemStatus {
    Waiting,
    Compiling,
    CE,
    Compiled,
    Running,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum TestCaseStatus {
    Running,
    RE,
    #[allow(clippy::upper_case_acronyms)]
    TLE,
    #[allow(clippy::upper_case_acronyms)]
    MLE,
    WA,
    AC,
    #[allow(unused)]
    #[allow(clippy::upper_case_acronyms)]
    UKE,
    CE,
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

fn string_to_command(command_str: &str) -> Result<Command, Box<dyn std::error::Error>> {
    let parts = shellwords::split(command_str)?;

    if parts.is_empty() {
        return Err("Empty command".into());
    }

    let mut cmd = Command::new(&parts[0]);

    if parts.len() > 1 {
        cmd.args(&parts[1..]);
    }

    Ok(cmd)
}

fn run_test_case(
    program_path: &Path,
    problem_name: &String,
    input_path: &Path,
    time_limit_ms: u128,
    memory_limit_bytes: u64,
) -> Result<TestCaseStatus, Box<dyn std::error::Error>> {
    // 复制输入文件
    let program_dir = program_path.parent().unwrap();
    let test_input_path = program_dir.join(format!("{}.in", problem_name));
    fs::copy(input_path, &test_input_path)?;

    // 启动程序
    let child = SharedChild::spawn(
        Command::new(program_path)
            .current_dir(program_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped()),
    )?;

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
) -> Result<TestCaseStatus, Box<dyn std::error::Error>> {
    let output_path = program_dir.join(format!("{}.out", problem_name));
    let input_path = program_dir.join(format!("{}.in", problem_name));

    // 检查输出文件是否存在
    if !output_path.exists() {
        return Ok(TestCaseStatus::WA);
    }

    // 复制答案文件
    let ans_path = program_dir.join(format!("{}.ans", problem_name));
    fs::copy(answer_path, &ans_path)?;

    // 查找第一个存在的 checker 文件
    let checker_path = get_context()
        .assets_dirs
        .iter()
        .find_map(|dir| {
            dir.join("checkers")
                .join("normal")
                .exists()
                .then(|| dir.join("checkers").join("normal"))
        })
        .unwrap_or_else(|| get_context().assets_dirs[0].join("checkers").join("normal"));

    // 使用校验器验证
    let validate_code = Command::new(&checker_path)
        .arg(&input_path)
        .arg(&output_path)
        .arg(&ans_path)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()?
        .code();

    match validate_code {
        Some(0) => Ok(TestCaseStatus::AC),
        _ => Ok(TestCaseStatus::WA),
    }
}

fn check_test_case(test_case: &TestCase, actual_score: u32) -> bool {
    let conditions = match &test_case.expected {
        ExpectedScore::Single(cond) => vec![cond.clone()],
        ExpectedScore::Multiple(conds) => conds.clone(),
    };

    let mut context: HashMapContext = HashMapContext::new();
    context
        .set_value("score".into(), Value::Int(actual_score as i64))
        .unwrap();

    for condition in &conditions {
        let expr = format!("score {}", condition);

        debug!("条件：{}", expr);

        if !eval_boolean_with_context_mut(&expr, &mut context).unwrap_or(false) {
            return false;
        }
    }

    true
}

// 将测试结果写入 CSV
fn write_results_to_csv(
    results: Vec<ProblemTestResult>,
    problem_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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

fn check_compiler(language: &Language) -> Result<(), Box<dyn std::error::Error>> {
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
            return Err(format!("{} 命令执行失败", &compiler.executable).into());
        }
    }
    Ok(())
}

pub fn main(_: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
    let (config, current_location) = get_context().config.as_ref().ok_or("找不到配置文件")?;

    let skip_level = match current_location {
        CurrentLocation::Problem(_, _) => 2,
        CurrentLocation::Day(_) => 1,
        _ => 0,
    };

    // 如果当前在Problem级别，还需要获取具体的day和problem名称
    let (target_day_name, target_problem_name) = match current_location {
        CurrentLocation::Problem(day_name, problem_name) => (Some(day_name), Some(problem_name)),
        CurrentLocation::Day(day_name) => (Some(day_name), None),
        _ => (None, None),
    };

    // 计算总任务数
    let total_days = if skip_level >= 1 {
        1
    } else {
        config.subconfig.len()
    };

    let day_pb = if skip_level >= 1 {
        // 如果跳过天级别，就不显示进度条
        get_context().multiprogress.add(ProgressBar::new(0))
    } else {
        get_context()
            .multiprogress
            .add(ProgressBar::new(total_days as u64))
    };

    if skip_level < 1 {
        day_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.green/blue}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );
    }

    let mut day_count = 0;
    for day in &config.subconfig {
        // 如果设置了跳过年份，并且这不是目标年份，则跳过
        if skip_level >= 1
            && target_day_name.is_some()
            && day.name != *target_day_name.as_ref().unwrap().to_string()
        {
            continue;
        }

        day_count += 1;
        if skip_level < 1 {
            day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
        }
        info!("处理天: {}", day.name);

        // 添加问题进度条
        let problem_count_in_day = if skip_level >= 2 {
            1
        } else {
            day.subconfig.len()
        };

        let problem_pb = if skip_level >= 2 {
            // 如果跳过问题级别，就不显示进度条
            get_context().multiprogress.add(ProgressBar::new(0))
        } else {
            get_context()
                .multiprogress
                .add(ProgressBar::new(problem_count_in_day as u64))
        };

        if skip_level < 2 {
            problem_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.cyan/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );
        }

        let mut problem_count = 0;
        for problem in &day.subconfig {
            // 如果设置了跳过问题，并且这不是目标问题，则跳过
            if skip_level >= 2
                && target_problem_name.is_some()
                && problem.name != *target_problem_name.as_ref().unwrap().to_string()
            {
                continue;
            }

            problem_count += 1;
            if skip_level < 2 {
                problem_pb.set_message(format!(
                    "处理第 {}/{} 题",
                    problem_count, problem_count_in_day
                ));
            }
            info!("处理题目: {}", problem.name);

            // 添加测试进度条
            let test_pb = get_context()
                .multiprogress
                .add(ProgressBar::new(problem.data.len() as u64));
            test_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.magenta/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

            // 收集所有测试者的结果
            let mut all_test_results = Vec::new();

            // 添加测试者进度条
            let tester_pb = get_context()
                .multiprogress
                .add(ProgressBar::new(problem.tests.len() as u64));
            tester_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.yellow/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

            let mut tester_count = 0;
            for (test_name, test) in &problem.tests {
                tester_count += 1;
                tester_pb.set_message(format!(
                    "处理第 {}/{} 个测试者: {}",
                    tester_count,
                    problem.tests.len(),
                    test_name
                ));

                info!("测试 {} 的程序", test_name);

                // 解析路径
                let path = if PathBuf::from_str(&test.path)?.is_absolute() {
                    PathBuf::from_str(&test.path)?
                } else {
                    problem.path.join(&test.path).canonicalize()?
                };

                info!("文件路径：{}", path.display());

                #[allow(unused_assignments)]
                let mut problem_status = ProblemStatus::Waiting;

                // 创建临时目录
                let tmp_dir = path.parent().unwrap().join("tmp");
                create_or_clear_dir(&tmp_dir)?;

                let target_path = tmp_dir.join(path.file_name().unwrap());
                fs::copy(&path, &target_path)?;

                // 编译
                #[allow(unused_assignments)]
                {
                    problem_status = ProblemStatus::Compiling;
                }
                info!("正在编译...");

                let ext = target_path
                    .extension()
                    .ok_or("文件无后缀名")?
                    .to_string_lossy();

                let file_type = get_context()
                    .languages
                    .get(ext.as_ref())
                    .ok_or("未知格式文件")?;

                check_compiler(file_type)?;

                let compile_status = string_to_command(&format!(
                    " {} {} {} {} {}",
                    file_type.compiler.executable,
                    file_type.compiler.object_set_arg,
                    &problem.name,
                    &day.compile.get(ext.as_ref()).ok_or("未知格式文件")?,
                    target_path.file_name().unwrap().to_string_lossy()
                ))?
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

                fs::remove_file(&target_path)?;
                let mut total_score = 0;

                // 运行测试用例
                if problem_status == ProblemStatus::Compiled {
                    #[allow(unused_assignments)]
                    {
                        problem_status = ProblemStatus::Running;
                    }
                    let program_path = tmp_dir.join(&problem.name);

                    // 用于存储单个测试用例结果的数组
                    let mut individual_results = Vec::new();
                    let mut case_count = 0;

                    // 添加测试用例进度条
                    let case_test_pb = get_context()
                        .multiprogress
                        .add(ProgressBar::new(problem.data.len() as u64));
                    case_test_pb.set_style(
                        indicatif::ProgressStyle::default_bar()
                            .template("  [{bar:40.magenta/blue}] {msg}")
                            .unwrap()
                            .progress_chars("=> "),
                    );

                    for case in &problem.data {
                        case_count += 1;

                        let input_path =
                            problem.path.join("data").join(case.input.as_ref().unwrap());
                        let answer_path = problem
                            .path
                            .join("data")
                            .join(case.output.as_ref().unwrap());

                        info!("运行测试点: {}", case.id);

                        // 运行程序
                        let run_result = run_test_case(
                            &program_path,
                            &problem.name,
                            &input_path,
                            (problem.time_limit * 1000.0) as u128, // 时间限制
                            ByteSize::from_str(&problem.memory_limit) // 空间限制
                                .unwrap_or_else(|e| {
                                    warn!(
                                        "空间限制东西字符串转换失败: {}, 使用 512 MiB 作为默认值",
                                        e
                                    );
                                    ByteSize::mib(512)
                                })
                                .as_u64(),
                        )?;

                        let case_status = match run_result {
                            TestCaseStatus::Running => {
                                // 程序正常结束，验证输出
                                validate_output(&tmp_dir, &problem.name, &answer_path)?
                            }
                            status => status,
                        };

                        info!("测试点结果: {:?}", case_status);

                        // 计分
                        let earned_score = if case_status == TestCaseStatus::AC {
                            case.score
                        } else {
                            0
                        };
                        total_score += earned_score;

                        // 记录单个测试用例结果
                        individual_results.push(IndividualTestCaseResult {
                            test_case_id: case.id,
                            status: case_status,
                            score: earned_score,
                            max_score: case.score,
                        });

                        // 更新进度条消息以显示当前测试点结果
                        let status_str = match case_status {
                            TestCaseStatus::AC => "AC".green(),
                            TestCaseStatus::WA => "WA".red(),
                            TestCaseStatus::TLE => "TLE".blue(),
                            TestCaseStatus::MLE => "MLE".blue(),
                            TestCaseStatus::RE => "RE".bright_blue(),
                            TestCaseStatus::UKE => "UKE".bright_black(),
                            TestCaseStatus::Running => "Running".yellow(),
                            TestCaseStatus::CE => "CE".yellow(),
                        };
                        case_test_pb.set_message(format!(
                            "运行测试点: {}/{} | #{} {}",
                            case_count,
                            problem.data.len(),
                            case.id,
                            status_str
                        ));
                        case_test_pb.inc(1);
                    }

                    case_test_pb.finish_and_clear();

                    // 为此题目创建测试结果
                    let problem_result = ProblemTestResult {
                        tester_name: test_name.to_string(),
                        test_case_results: individual_results,
                        total_score,
                        max_possible_score: problem.data.iter().map(|case| case.score).sum(),
                    };

                    // 将结果添加到收集向量中
                    all_test_results.push(problem_result);
                } else {
                    // 如果编译失败，创建一个空的结果
                    let problem_result = ProblemTestResult {
                        tester_name: test_name.to_string(),
                        test_case_results: vec![IndividualTestCaseResult {
                            test_case_id: 0,
                            status: TestCaseStatus::CE,
                            score: 0,
                            max_score: problem.data.iter().map(|case| case.score).sum(),
                        }],
                        total_score: 0,
                        max_possible_score: problem.data.iter().map(|case| case.score).sum(),
                    };
                    all_test_results.push(problem_result);
                }

                info!(
                    "总得分: {}/{}",
                    total_score,
                    problem.data.iter().map(|case| case.score).sum::<u32>()
                );

                if check_test_case(test, total_score) {
                    info!("测试 {} 通过", test_name);
                } else {
                    warn!("测试 {} 不满足所有条件", test_name);
                }

                tester_pb.inc(1);

                // 清理临时文件
                let _ = fs::remove_dir_all(&tmp_dir);
            }

            tester_pb.finish_and_clear();

            // 在所有测试完成后，将所有结果写入CSV文件
            write_results_to_csv(all_test_results, &problem.path)?;

            if skip_level == 2 {
                test_pb.finish_with_message("测试完成！");
            } else {
                test_pb.finish_and_clear();
            }

            // 如果是特定问题，处理完后就跳出循环
            if skip_level >= 2 {
                break;
            }
        }

        // if !skip_level >= 2 {
        //     problem_pb.finish_and_clear();
        // }

        if skip_level == 1 {
            problem_pb.finish_with_message("测试完成！");
        } else {
            problem_pb.finish_and_clear();
        }

        // 如果是特定天，处理完后就跳出循环
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
