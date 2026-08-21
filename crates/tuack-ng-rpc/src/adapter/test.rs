use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use tuack_config::lang::Language;
use tuack_config::{
    ContestDayConfig, ProblemConfig, ScorePolicy as ConfigScorePolicy, SubtaskItem,
};
use tuack_lib::test::{TaskParams, TestCaseStatus, TestSession};
use tuack_lib::utils::compiler::Runner;
use tuack_lib::utils::testlib::Checker;
use tuack_utils::checkers::cpp::CppChecker;
use tuack_utils::checkers::prebuilt::PrebuiltChecker;
use tuack_utils::compilers::cpp::CppRunner;
use tuack_utils::compilers::general::GeneralRunner;
use tuack_utils::data::{problem_sample_data, problem_test_data};

use crate::jsonrpc::{COMPILE_ERROR, INTERNAL_ERROR, INVALID_PARAMS, RUN_ERROR, RpcError};
use crate::protocol::{Target, events};
use crate::session::{EventEmitter, RpcContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Preparing,
    Ready,
    Cancelled,
    Error,
    Closed,
}

impl RunState {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunState::Preparing => "preparing",
            RunState::Ready => "ready",
            RunState::Cancelled => "cancelled",
            RunState::Error => "error",
            RunState::Closed => "closed",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgeEntry {
    pub test_id: String,
    pub status: &'static str,
    pub time_ms: Option<u64>,
    pub memory_bytes: Option<u64>,
    pub message: Option<String>,
    pub score: f64,
    pub full_score: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupScore {
    pub id: u32,
    pub earned: u32,
    pub full: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreReport {
    pub groups: Vec<GroupScore>,
    pub total: u32,
    pub full_score: u32,
}

/// 已编译就绪的评测会话
pub struct RunSession {
    pub problem: ProblemConfig,
    pub params: TaskParams,
    pub runner: Box<dyn Runner>,
    pub checker: Box<dyn Checker>,
}

/// run 句柄（评测会话状态 + 取消）
pub struct RunHandle {
    pub id: String,
    pub session_id: String,
    pub problem_id: String,
    pub target: Target,
    /// 被测代码标识（`tests` 的 key，缺省 `std`）
    pub tester: String,
    pub cancel: watch::Sender<bool>,
    pub state: Mutex<RunState>,
    pub error: Mutex<Option<String>>,
    pub judged: Mutex<Vec<JudgeEntry>>,
    pub report: Mutex<Option<ScoreReport>>,
    pub session: tokio::sync::Mutex<Option<RunSession>>,
}

impl RunHandle {
    pub fn new(
        id: String,
        session_id: String,
        problem_id: String,
        target: Target,
        tester: String,
    ) -> Self {
        let (cancel, _rx) = watch::channel(false);
        RunHandle {
            id,
            session_id,
            problem_id,
            target,
            tester,
            cancel,
            state: Mutex::new(RunState::Preparing),
            error: Mutex::new(None),
            judged: Mutex::new(Vec::new()),
            report: Mutex::new(None),
            session: tokio::sync::Mutex::new(None),
        }
    }
}

fn status_str(s: &TestCaseStatus) -> &'static str {
    match s {
        TestCaseStatus::AC => "AC",
        TestCaseStatus::WA => "WA",
        TestCaseStatus::RE => "RE",
        TestCaseStatus::TLE => "TLE",
        TestCaseStatus::MLE => "MLE",
        TestCaseStatus::UKE => "UKE",
        TestCaseStatus::FE => "FE",
        TestCaseStatus::PC(_) => "PC",
    }
}

fn fail(handle: &RunHandle, ctx: &RpcContext, msg: &str) {
    *handle.state.lock().unwrap() = RunState::Error;
    *handle.error.lock().unwrap() = Some(msg.to_string());
    ctx.emitter.emit(
        "run/finished",
        events::run_finished(
            &handle.session_id,
            &handle.id,
            "error",
            Some(msg.to_string()),
        ),
    );
}

fn cancelled(handle: &RunHandle, ctx: &RpcContext) {
    *handle.state.lock().unwrap() = RunState::Cancelled;
    ctx.emitter.emit(
        "run/finished",
        events::run_finished(&handle.session_id, &handle.id, "cancelled", None),
    );
}

/// 编译 checker（SPJ 或默认 normal），输出经 compiler 通道通知
async fn build_checker(
    handle: &RunHandle,
    problem: &ProblemConfig,
    assets_dirs: &[std::path::PathBuf],
    emitter: &EventEmitter,
) -> Result<Box<dyn Checker>, String> {
    match &problem.checker {
        Some(pair) => {
            let chk_config = if handle.target == Target::Sample {
                pair.sample.as_ref().unwrap_or(&pair.data)
            } else {
                &pair.data
            };
            let source_path = problem.path.join(&chk_config.source);
            if !source_path.exists() {
                return Err("Checker 源文件不存在".to_string());
            }
            let mut deps: IndexMap<String, Vec<u8>> = IndexMap::new();
            for dep in &chk_config.deps {
                let abs = problem.path.join(dep);
                let content =
                    std::fs::read(&abs).map_err(|e| format!("Checker 依赖读取失败：{}", e))?;
                let name = abs.file_name().unwrap().to_string_lossy().to_string();
                deps.insert(name, content);
            }
            let mut cpp = CppChecker::new(&source_path, &IndexMap::new(), "chk", deps)
                .map_err(|e| e.to_string())?;
            let (cpp, prepared) = tokio::task::spawn_blocking(move || {
                let r = cpp.prepare();
                (cpp, r)
            })
            .await
            .map_err(|e| format!("Checker 编译任务失败：{}", e))?;
            if let Err(e) = prepared {
                let msg = format!("{:#}", e);
                emitter.emit(
                    "run/output",
                    events::run_output(&handle.session_id, &handle.id, None, "compiler", &msg),
                );
                return Err("Checker 编译失败".to_string());
            }
            Ok(Box::new(cpp))
        }
        None => {
            let binary = assets_dirs
                .iter()
                .find_map(|d| {
                    let p = d
                        .join("checkers")
                        .join(format!("normal{}", std::env::consts::EXE_SUFFIX));
                    p.exists().then_some(p)
                })
                .ok_or_else(|| "默认 Checker 文件不存在".to_string())?;
            Ok(Box::new(PrebuiltChecker::new(binary)))
        }
    }
}

/// 编译 std 代码运行器，输出经 compiler 通道通知
async fn build_runner(
    handle: &RunHandle,
    problem: &ProblemConfig,
    day: &ContestDayConfig,
    languages: &IndexMap<String, Language>,
    emitter: &EventEmitter,
) -> Result<Box<dyn Runner>, String> {
    let test = problem
        .tests
        .get(&handle.tester)
        .or_else(|| problem.tests.iter().next().map(|(_, t)| t))
        .ok_or_else(|| "题目没有配置测试代码".to_string())?;

    let path = if PathBufFrom::is_absolute(&test.path) {
        std::path::PathBuf::from_str(&test.path).unwrap()
    } else {
        dunce::canonicalize(problem.path.join(&test.path))
            .map_err(|e| format!("路径解析失败：{}", e))?
    };

    let ext = path
        .extension()
        .ok_or_else(|| "文件无后缀名".to_string())?
        .to_string_lossy()
        .to_string();

    let mut runner: Box<dyn Runner> = if ext == "cpp" {
        Box::new(
            CppRunner::new(&path, &day.compile, problem.name.clone()).map_err(|e| e.to_string())?,
        )
    } else {
        Box::new(
            GeneralRunner::new(&path, &day.compile, problem.name.clone(), languages)
                .map_err(|e| e.to_string())?,
        )
    };

    // 交互题：装配 grader 与 header（须在 prepare 之前）
    if problem.problem_type == tuack_config::ProblemType::Interactive {
        if !runner.manifest().interactive {
            return Err("该语言不支持交互".to_string());
        }
        let interactive = problem
            .interactive
            .as_ref()
            .ok_or_else(|| "交互题目缺少 interactive 配置".to_string())?;
        let resolve = |path: &String| -> Result<std::path::PathBuf, String> {
            let p = std::path::PathBuf::from_str(path).map_err(|e| e.to_string())?;
            if p.is_absolute() {
                Ok(p)
            } else {
                dunce::canonicalize(problem.path.join(p))
                    .map_err(|e| format!("路径解析失败：{}", e))
            }
        };
        let grader_path = if handle.target == Target::Sample {
            match &interactive.sample_grader {
                Some(sg) => resolve(sg)?,
                None => resolve(&interactive.grader)?,
            }
        } else {
            resolve(&interactive.grader)?
        };
        let header_path = resolve(&interactive.header)?;
        if !grader_path.exists() {
            return Err("grader 不存在".to_string());
        }
        if !header_path.exists() {
            return Err("header 不存在".to_string());
        }
        runner
            .set_interactive(&grader_path, &header_path)
            .map_err(|e| e.to_string())?;
    }

    if let Err(e) = runner.prepare_async().await {
        let msg = format!("{:#}", e);
        emitter.emit(
            "run/output",
            events::run_output(&handle.session_id, &handle.id, None, "compiler", &msg),
        );
        return Err(format!("{} 代码编译失败", handle.tester));
    }
    Ok(runner)
}

struct PathBufFrom;

impl PathBufFrom {
    fn is_absolute(s: &str) -> bool {
        std::path::PathBuf::from_str(s)
            .map(|p| p.is_absolute())
            .unwrap_or(false)
    }
}

/// 异步准备：编译 checker + std 代码，就绪后发 run/ready
pub async fn prepare(
    ctx: Arc<RpcContext>,
    handle: Arc<RunHandle>,
    problem_config: ProblemConfig,
    day_config: ContestDayConfig,
) {
    let checker =
        match build_checker(&handle, &problem_config, &ctx.assets_dirs, &ctx.emitter).await {
            Ok(c) => c,
            Err(msg) => {
                fail(&handle, &ctx, &msg);
                return;
            }
        };
    let runner = match build_runner(
        &handle,
        &problem_config,
        &day_config,
        &ctx.languages,
        &ctx.emitter,
    )
    .await
    {
        Ok(r) => r,
        Err(msg) => {
            fail(&handle, &ctx, &msg);
            return;
        }
    };
    if *handle.cancel.borrow() {
        cancelled(&handle, &ctx);
        return;
    }

    let params = TaskParams {
        problem_name: problem_config.name.clone(),
        time_limit: Duration::from_secs_f64(problem_config.time_limit),
        memory_limit: problem_config.memory_limit,
        file_io: problem_config.file_io.unwrap_or(true),
    };
    let session = RunSession {
        problem: problem_config,
        params,
        runner,
        checker,
    };
    *handle.session.lock().await = Some(session);
    *handle.state.lock().unwrap() = RunState::Ready;
    ctx.emitter.emit(
        "run/ready",
        events::run_ready(&handle.session_id, &handle.id),
    );
}

/// 单点评测（同步阻塞）；judge 相关的输出事件先于本请求的响应
pub async fn judge(
    ctx: &RpcContext,
    handle: Arc<RunHandle>,
    test_id: String,
) -> Result<Value, RpcError> {
    match *handle.state.lock().unwrap() {
        RunState::Error => {
            let msg = handle.error.lock().unwrap().clone().unwrap_or_default();
            return Err(RpcError::new(COMPILE_ERROR, msg));
        }
        RunState::Cancelled | RunState::Closed => {
            return Err(RpcError::new(INVALID_PARAMS, "run 已终止"));
        }
        RunState::Preparing => {
            return Err(RpcError::new(INVALID_PARAMS, "run 尚未就绪"));
        }
        RunState::Ready => {}
    }
    if *handle.cancel.borrow() {
        return Err(RpcError::new(INVALID_PARAMS, "run 已取消"));
    }

    let mut guard = handle.session.lock().await;
    let rs = guard
        .as_mut()
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "run 尚未就绪"))?;

    let data_items = match handle.target {
        Target::Data => problem_test_data(&rs.problem),
        Target::Sample => problem_sample_data(&rs.problem),
    };
    let item = data_items
        .iter()
        .find(|d| d.id().to_string() == test_id)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("测试点不存在：{}", test_id)))?;
    let full_score = item.full_score();

    let mut ts = TestSession::new(rs.runner.as_mut(), rs.checker.as_ref(), rs.params.clone());
    let result = ts
        .judge(item)
        .await
        .map_err(|e| RpcError::new(RUN_ERROR, format!("评测失败：{:#}", e)))?;

    let status = status_str(&result.status);
    let time_ms = result.time.map(|d| d.as_millis() as u64);
    let memory_bytes = result.memory.map(|b| b.as_u64());
    let message = result.message.clone();
    let score = result.score;

    if let Some(msg) = &message {
        if !msg.trim().is_empty() {
            ctx.emitter.emit(
                "run/output",
                events::run_output(
                    &handle.session_id,
                    &handle.id,
                    Some(test_id.clone()),
                    "judge",
                    msg,
                ),
            );
        }
    }

    handle.judged.lock().unwrap().push(JudgeEntry {
        test_id: test_id.clone(),
        status,
        time_ms,
        memory_bytes,
        message: message.clone(),
        score,
        full_score,
    });

    Ok(json!({
        "testId": test_id,
        "status": status,
        "timeMs": time_ms,
        "memoryBytes": memory_bytes,
        "message": message,
        "score": score,
        "fullScore": full_score,
    }))
}

/// 对已评测点做 subtask 判分汇总
pub async fn score(handle: Arc<RunHandle>) -> Result<Value, RpcError> {
    match *handle.state.lock().unwrap() {
        RunState::Preparing => return Err(RpcError::new(INVALID_PARAMS, "run 尚未就绪")),
        RunState::Cancelled | RunState::Closed => {
            return Err(RpcError::new(INVALID_PARAMS, "run 已终止"));
        }
        _ => {}
    }

    let guard = handle.session.lock().await;
    let rs = guard
        .as_ref()
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "run 尚未就绪"))?;
    let judged = handle.judged.lock().unwrap().clone();

    let mut case_scores: Vec<(u32, u32)> = Vec::new();
    for entry in &judged {
        let (subtask, full) = match handle.target {
            Target::Data => {
                let item = rs
                    .problem
                    .runtime
                    .data
                    .iter()
                    .find(|d| d.id.to_string() == entry.test_id)
                    .ok_or_else(|| {
                        RpcError::new(INVALID_PARAMS, format!("测试点不存在：{}", entry.test_id))
                    })?;
                (item.subtask, item.score)
            }
            Target::Sample => (0u32, 1u32),
        };
        let earned = (entry.score * full as f64).round() as u32;
        case_scores.push((subtask, earned));
    }

    let groups = match handle.target {
        Target::Data => rs.problem.runtime.subtasks.clone(),
        Target::Sample => {
            let n = rs.problem.runtime.samples.len() as u32;
            BTreeMap::from([(
                0u32,
                SubtaskItem {
                    items: vec![],
                    max_score: n,
                    policy: ConfigScorePolicy::Sum,
                },
            )])
        }
    };
    let (group_out, total, full_score) = aggregate(&groups, &case_scores);
    let report = ScoreReport {
        groups: group_out,
        total,
        full_score,
    };
    *handle.report.lock().unwrap() = Some(report.clone());

    let judged_count = judged.len();
    let total_points = match handle.target {
        Target::Data => rs.problem.runtime.data.len(),
        Target::Sample => rs.problem.runtime.samples.len(),
    };
    let report_value = serde_json::to_value(&report)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("判分序列化失败：{}", e)))?;
    Ok(json!({ "judged": judged_count, "total": total_points, "report": report_value }))
}

pub fn cancel(ctx: &RpcContext, handle: Arc<RunHandle>) {
    let mut state = handle.state.lock().unwrap();
    if matches!(*state, RunState::Preparing | RunState::Ready) {
        let _ = handle.cancel.send(true);
        *state = RunState::Cancelled;
        ctx.emitter.emit(
            "run/finished",
            events::run_finished(&handle.session_id, &handle.id, "cancelled", None),
        );
    }
}

pub async fn get(handle: Arc<RunHandle>) -> Result<Value, RpcError> {
    let state = *handle.state.lock().unwrap();
    let judged = handle.judged.lock().unwrap().clone();
    let report = handle.report.lock().unwrap().clone();
    let error = handle.error.lock().unwrap().clone();

    let judged_value = serde_json::to_value(&judged)
        .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("序列化失败：{}", e)))?;
    let report_value = match report {
        Some(r) => serde_json::to_value(&r)
            .map_err(|e| RpcError::new(INTERNAL_ERROR, format!("序列化失败：{}", e)))?,
        None => Value::Null,
    };

    Ok(json!({
        "state": state.as_str(),
        "problem": handle.problem_id,
        "target": handle.target.as_str(),
        "tester": handle.tester,
        "judged": judged_value,
        "report": report_value,
        "error": error.map(Value::String).unwrap_or(Value::Null),
    }))
}

fn aggregate(
    groups: &BTreeMap<u32, SubtaskItem>,
    case_scores: &[(u32, u32)],
) -> (Vec<GroupScore>, u32, u32) {
    let mut by_group: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (subtask, earned) in case_scores {
        by_group.entry(*subtask).or_default().push(*earned);
    }

    let mut out = Vec::new();
    let mut total = 0;
    let mut full_score = 0;
    for (id, group) in groups {
        let scores = by_group.get(id).cloned().unwrap_or_default();
        let earned = match group.policy {
            ConfigScorePolicy::Sum => scores.iter().sum(),
            ConfigScorePolicy::Max => scores.iter().max().copied().unwrap_or(0),
            ConfigScorePolicy::Min => scores.iter().min().copied().unwrap_or(0),
        };
        total += earned;
        full_score += group.max_score;
        out.push(GroupScore {
            id: *id,
            earned,
            full: group.max_score,
        });
    }
    (out, total, full_score)
}
