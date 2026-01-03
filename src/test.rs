use crate::{config::load_config, context::get_context};
use clap::Args;
use log::{debug, error, info};
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
    input_path: &Path,
    time_limit_ms: u128,
    memory_limit_bytes: u64,
) -> Result<TestCaseStatus, Box<dyn std::error::Error>> {
    // 复制输入文件
    let program_dir = program_path.parent().unwrap();
    let test_input_path = program_dir.join("test.in");
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
            thread::sleep(Duration::from_millis(10));
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
    let input_path = program_dir.join("test.in");

    // 检查输出文件是否存在
    if !output_path.exists() {
        return Ok(TestCaseStatus::UKE);
    }

    // 复制答案文件
    let ans_path = program_dir.join(format!("{}.ans", problem_name));
    fs::copy(answer_path, &ans_path)?;

    // 使用校验器验证
    let validate_code = Command::new(&get_context().assets_dirs[0].join("checkers").join("normal"))
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

    for day in config.subconfig {
        for problem in day.subconfig {
            for (test_name, test) in problem.tests {
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
                problem_status = ProblemStatus::Compiling;
                info!("正在编译...");

                let compile_status = Command::new("g++")
                    .current_dir(&tmp_dir)
                    .arg("-o")
                    .arg(&problem.name)
                    .arg(target_path.file_name().unwrap())
                    .status()?;

                if compile_status.success() {
                    problem_status = ProblemStatus::Compiled;
                    info!("编译成功");
                } else {
                    problem_status = ProblemStatus::CE;
                    info!("编译错误");
                    // continue;
                }

                fs::remove_file(&target_path)?;

                // 运行测试用例
                if problem_status == ProblemStatus::Compiled {
                    problem_status = ProblemStatus::Running;

                    let program_path = tmp_dir.join(&problem.name);
                    let mut total_score = 0;

                    for case in &problem.data {
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
                            &input_path,
                            1000,              // 1秒时间限制
                            512 * 1024 * 1024, // 512MB内存限制
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
                        if case_status == TestCaseStatus::AC {
                            total_score += case.score;
                        }
                    }

                    info!(
                        "总得分: {}/{}",
                        total_score,
                        problem.data.iter().map(|case| case.score).sum::<u32>()
                    );

                    // 清理临时文件
                    let _ = fs::remove_dir_all(&tmp_dir);
                }
            }
        }
    }

    Ok(())
}
