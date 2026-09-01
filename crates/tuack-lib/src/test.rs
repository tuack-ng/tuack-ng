use std::time::Duration;

use bytesize::ByteSize;

use crate::data::Data;
use crate::prelude::*;
use crate::utils::compiler::{IoMode, ResourceLimits, RunStatus, Runner};
use crate::utils::testlib::{Checker, JudgeResult};

/// 测试点评测状态。
#[derive(Debug)]
#[allow(clippy::upper_case_acronyms)]
pub enum TestCaseStatus {
    AC,
    WA,
    RE,
    TLE,
    MLE,
    UKE,
    /// 文件错误：输出文件不存在
    FE,
    /// 部分正确，携带 0-100 的比例分
    PC(f64),
}

/// 单个测试点评测结果。
#[derive(Debug)]
pub struct TestCaseResult {
    pub status: TestCaseStatus,
    /// 归一化得分，`AC=1.0`、`PC=p/100`、其余 `0.0`
    pub score: f64,
    pub time: Option<Duration>,
    pub memory: Option<ByteSize>,
    pub message: Option<String>,
}

/// 运行参数。
#[derive(Debug, Clone)]
pub struct TaskParams {
    pub problem_name: String,
    pub time_limit: Duration,
    pub memory_limit: ByteSize,
    pub file_io: bool,
}

/// 测试会话。
#[allow(unused)]
pub struct TestSession<'a> {
    runner: &'a mut dyn Runner,
    checker: &'a dyn Checker,
    params: TaskParams,
}

/// 构造一个 UKE 结果（例如输入/答案文件读取失败）。
fn uke_result(message: String) -> TestCaseResult {
    TestCaseResult {
        status: TestCaseStatus::UKE,
        score: 0.0,
        time: None,
        memory: None,
        message: Some(message),
    }
}

impl<'a> TestSession<'a> {
    pub fn new(runner: &'a mut dyn Runner, checker: &'a dyn Checker, params: TaskParams) -> Self {
        Self {
            runner,
            checker,
            params,
        }
    }

    /// 评测单个测试点：设置 limits/io_mode -> 注入输入 -> 执行 -> 校验 -> 返回结果。
    pub async fn judge(&mut self, data: &dyn Data) -> Result<TestCaseResult> {
        self.runner.set_limits(ResourceLimits::new(
            self.params.time_limit,
            self.params.memory_limit.as_u64(),
        ));
        self.runner.set_io_mode(if self.params.file_io {
            IoMode::File {
                input_name: format!("{}.in", self.params.problem_name),
                output_name: format!("{}.out", self.params.problem_name),
            }
        } else {
            IoMode::Stdio
        });

        let input = match data.input().await {
            Ok(i) => i,
            Err(e) => return Ok(uke_result(format!("读取输入失败：{e}"))),
        };
        self.runner.set_input(input);
        let run = self.runner.execute().await?;

        let (status, score, message) = match (run.status, run.output) {
            (RunStatus::Success, None) => {
                (TestCaseStatus::FE, 0.0, Some("未找到输出文件".to_string()))
            }
            (RunStatus::Success, Some(mut output)) => {
                let mut input = match data.input().await {
                    Ok(i) => i,
                    Err(e) => return Ok(uke_result(format!("读取输入失败：{e}"))),
                };
                let mut answer = match data.answer().await {
                    Ok(a) => a,
                    Err(e) => return Ok(uke_result(format!("读取答案失败：{e}"))),
                };
                match self
                    .checker
                    .validate(&mut input, &mut output, &mut answer)
                    .await
                {
                    Ok((JudgeResult::Accepted, msg)) => (TestCaseStatus::AC, 1.0, Some(msg)),
                    Ok((JudgeResult::WrongAnswer, msg)) => (TestCaseStatus::WA, 0.0, Some(msg)),
                    Ok((JudgeResult::PresentationError, msg)) => {
                        (TestCaseStatus::WA, 0.0, Some(msg))
                    }
                    Ok((JudgeResult::Score(p), msg)) => (
                        TestCaseStatus::PC(p),
                        (p / 100.0).clamp(0.0, 1.0),
                        Some(msg),
                    ),
                    Ok((JudgeResult::Fail, msg)) => (TestCaseStatus::UKE, 0.0, Some(msg)),
                    Err(e) => (TestCaseStatus::UKE, 0.0, Some(format!("{e:#}"))),
                }
            }
            (RunStatus::NonZeroExit(_), _) => (TestCaseStatus::RE, 0.0, None),
            (RunStatus::TimeLimitExceeded, _) => (TestCaseStatus::TLE, 0.0, None),
            (RunStatus::MemoryLimitExceeded, _) => (TestCaseStatus::MLE, 0.0, None),
            (RunStatus::InternalError(e), _) => (TestCaseStatus::UKE, 0.0, Some(format!("{e:#}"))),
        };

        Ok(TestCaseResult {
            status,
            score,
            time: run.time,
            memory: run.memory.map(ByteSize),
            message,
        })
    }
}
