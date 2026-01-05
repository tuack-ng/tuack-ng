use crate::config::{ExpectedScore, TestCase};
use crate::{config::load_config, context::get_context};
use bytesize::ByteSize;
use clap::Args;
use colored::Colorize;
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
    TLE,
    MLE,
    WA,
    AC,
    #[allow(unused)]
    UKE,
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
        let pid = Pid::from_u32(child_clone.id() as u32);

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
            if let Some(process) = sys.process(pid) {
                if process.memory() > memory_limit_bytes {
                    let _ = child_clone.kill();
                    *status_clone.lock().unwrap() = TestCaseStatus::MLE;
                    break;
                }
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

pub fn main(_: TestArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 检查编译环境
    debug!("检查 C++ 编译环境");
    match Command::new("g++").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version_output = String::from_utf8_lossy(&output.stdout);
            let version = version_output.lines().next().unwrap_or("").trim();
            debug!("g++ 版本: {}", version);
        }
        _ => {
            error!("未找到 g++ 命令，请确保已安装并添加到PATH");
            return Err("g++ 命令执行失败".into());
        }
    }

    let config = load_config(Path::new("."))?;

    // 计算总任务数
    let total_days = config.subconfig.len();
    let _total_problems: usize = config
        .subconfig
        .iter()
        .map(|day| {
            day.subconfig.len()
                * day
                    .subconfig
                    .iter()
                    .map(|problem| problem.tests.len())
                    .sum::<usize>()
        })
        .sum();
    let _total_test_cases: usize = config
        .subconfig
        .iter()
        .flat_map(|day| &day.subconfig)
        .flat_map(|problem| &problem.data)
        .count();

    // 添加进度条
    let multi = &get_context().multiprogress;
    let day_pb = multi.add(ProgressBar::new(total_days as u64));
    day_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  [{bar:40.green/blue}] {msg}")
            .unwrap()
            .progress_chars("=> "),
    );

    let mut day_count = 0;
    for day in &config.subconfig {
        day_count += 1;
        day_pb.set_message(format!("处理第 {}/{} 天", day_count, total_days));
        info!("处理天: {}", day.name);

        // 添加问题进度条
        let problem_pb = multi.add(ProgressBar::new(day.subconfig.len() as u64));
        problem_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("  [{bar:40.cyan/blue}] {msg}")
                .unwrap()
                .progress_chars("=> "),
        );

        let mut problem_count = 0;
        for problem in &day.subconfig {
            problem_count += 1;
            problem_pb.set_message(format!(
                "处理第 {}/{} 题",
                problem_count,
                day.subconfig.len()
            ));
            info!("处理题目: {}", problem.name);

            // 添加测试进度条
            let test_pb = multi.add(ProgressBar::new(problem.data.len() as u64));
            test_pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("  [{bar:40.magenta/blue}] {msg}")
                    .unwrap()
                    .progress_chars("=> "),
            );

            for (test_name, test) in &problem.tests {
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

                let compile_status = Command::new("g++")
                    .current_dir(&tmp_dir)
                    .arg("-o")
                    .arg(&problem.name)
                    .arg("-O2")
                    .arg("-std=c++14")
                    .arg(target_path.file_name().unwrap())
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

                    let mut case_count = 0;
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

                        info!(
                            "{}",
                            bytesize::ByteSize::from_str(&problem.memory_limit) // 空间限制
                                .unwrap_or_else(|e| {
                                    warn!(
                                        "空间限制东西字符串转换失败: {}, 使用 512 MiB 作为默认值",
                                        e
                                    );
                                    bytesize::ByteSize::mib(512)
                                })
                                .as_u64()
                        );

                        let case_status = match run_result {
                            TestCaseStatus::Running => {
                                // 程序正常结束，验证输出
                                validate_output(&tmp_dir, &problem.name, &answer_path)?
                            }
                            status => status,
                        };

                        info!("测试点结果: {:?}", case_status);

                        // 计分
                        if case_status == TestCaseStatus::AC {
                            total_score += case.score;
                        }

                        // 更新进度条消息以显示当前测试点结果
                        let status_str = match case_status {
                            TestCaseStatus::AC => "AC".green(),
                            TestCaseStatus::WA => "WA".red(),
                            TestCaseStatus::TLE => "TLE".blue(),
                            TestCaseStatus::MLE => "MLE".blue(),
                            TestCaseStatus::RE => "RE".bright_blue(),
                            TestCaseStatus::UKE => "UKE".bright_black(),
                            TestCaseStatus::Running => "Running".yellow(),
                        };
                        test_pb.set_message(format!(
                            "运行测试点: {}/{} | #{} {}",
                            case_count,
                            problem.data.len(),
                            case.id,
                            status_str
                        ));
                        test_pb.inc(1);
                    }
                }
                info!(
                    "总得分: {}/{}",
                    total_score,
                    problem.data.iter().map(|case| case.score).sum::<u32>()
                );

                if check_test_case(&test, total_score) {
                    info!("测试 {} 通过", test_name);
                } else {
                    warn!("测试 {} 不满足所有条件", test_name);
                }

                test_pb.finish_and_clear();

                // 清理临时文件
                let _ = fs::remove_dir_all(&tmp_dir);
            }
        }
        problem_pb.finish_and_clear();
    }
    day_pb.finish_with_message("测试完成！");

    Ok(())
}
