use serde::Deserialize;
use serde_json::{Value, json};

use tuack_config::ProblemType;

use crate::jsonrpc::{INVALID_CONFIG, INVALID_PARAMS, INVALID_PROJECT, RpcError};
use crate::protocol::{Scope, escape_segment, unescape_segment};
use crate::session::RpcContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListParams {
    #[allow(dead_code)]
    session_id: String,
    scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetParams {
    #[allow(dead_code)]
    session_id: String,
    problem: String,
}

fn problem_type_str(t: &ProblemType) -> &'static str {
    match t {
        ProblemType::Program => "program",
        ProblemType::Output => "output",
        ProblemType::Interactive => "interactive",
    }
}

fn info(day: &str, name: &str, problem: &tuack_config::ProblemConfig) -> Value {
    json!({
        "name": problem.name,
        "title": problem.title,
        "problemType": problem_type_str(&problem.problem_type),
        "path": format!("{}/{}", escape_segment(day), escape_segment(name)),
    })
}

pub async fn list(ctx: &RpcContext, params: Option<Value>) -> Result<Value, RpcError> {
    let params: ListParams = parse_params(params)?;
    let scope = match params.scope {
        Some(s) => Scope::parse(&s).map_err(|e| RpcError::new(INVALID_PARAMS, e))?,
        None => Scope::Contest,
    };
    let config = ctx
        .config
        .read()
        .await
        .clone()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;

    let mut problems = Vec::new();
    let contest = &config.config;
    match scope {
        Scope::Contest => {
            for (day_name, day) in contest.subconfig.iter() {
                for (prob_name, prob) in day.subconfig.iter() {
                    problems.push(info(day_name, prob_name, prob));
                }
            }
        }
        Scope::Day(day) => {
            let day_config = contest
                .subconfig
                .get(&day)
                .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day)))?;
            for (prob_name, prob) in day_config.subconfig.iter() {
                problems.push(info(&day, prob_name, prob));
            }
        }
        Scope::Problem { day, problem } => {
            let day_config = contest
                .subconfig
                .get(&day)
                .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day)))?;
            let prob = day_config.subconfig.get(&problem).ok_or_else(|| {
                RpcError::new(INVALID_CONFIG, format!("找不到 problem: {}", problem))
            })?;
            problems.push(info(&day, &problem, prob));
        }
    }
    Ok(json!({ "problems": problems }))
}

pub async fn get(ctx: &RpcContext, params: Option<Value>) -> Result<Value, RpcError> {
    let params: GetParams = parse_params(params)?;
    let config = ctx
        .config
        .read()
        .await
        .clone()
        .ok_or_else(|| RpcError::new(INVALID_PROJECT, "没有有效的工程"))?;

    let (day_name, prob_name) = parse_problem_id(&params.problem)?;
    let contest = &config.config;
    let day_config = contest
        .subconfig
        .get(&day_name)
        .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 day: {}", day_name)))?;
    let problem = day_config
        .subconfig
        .get(&prob_name)
        .ok_or_else(|| RpcError::new(INVALID_CONFIG, format!("找不到 problem: {}", prob_name)))?;

    let data: Vec<Value> = problem
        .runtime
        .data
        .iter()
        .map(|d| {
            json!({
                "id": d.id,
                "score": d.score,
                "subtask": d.subtask,
            })
        })
        .collect();

    let samples: Vec<Value> = problem
        .runtime
        .samples
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "input": s.input,
                "output": s.output,
            })
        })
        .collect();

    let checker = problem
        .checker
        .as_ref()
        .map(|c| {
            let data = json!({ "source": c.data.source, "deps": c.data.deps });
            let sample = c
                .sample
                .as_ref()
                .map(|s| json!({ "source": s.source, "deps": s.deps }));
            json!({ "data": data, "sample": sample })
        })
        .unwrap_or(Value::Null);

    let validator = problem
        .validator
        .as_ref()
        .map(|v| {
            let data = json!({ "source": v.data.source, "deps": v.data.deps });
            let sample = v
                .sample
                .as_ref()
                .map(|s| json!({ "source": s.source, "deps": s.deps }));
            json!({ "data": data, "sample": sample })
        })
        .unwrap_or(Value::Null);

    Ok(json!({
        "problem": {
            "name": problem.name,
            "title": problem.title,
            "problemType": problem_type_str(&problem.problem_type),
            "timeLimitMs": (problem.time_limit * 1000.0) as u64,
            "memoryLimitBytes": problem.memory_limit.as_u64(),
            "fileIo": problem.file_io,
            "data": data,
            "samples": samples,
            "checker": checker,
            "validator": validator,
            "path": format!("{}/{}", escape_segment(&day_name), escape_segment(&prob_name)),
        }
    }))
}

/// 解析绝对问题标识 `<day>/<problem>`（层级内分隔符按 `~1`/`~0` 转义）
fn parse_problem_id(id: &str) -> Result<(String, String), RpcError> {
    let parts: Vec<String> = id.split('/').map(unescape_segment).collect();
    match parts.as_slice() {
        [day, problem] if !day.is_empty() && !problem.is_empty() => {
            Ok((day.clone(), problem.clone()))
        }
        _ => Err(RpcError::new(
            INVALID_CONFIG,
            "问题标识应为 <day>/<problem>",
        )),
    }
}

fn parse_params<T: serde::de::DeserializeOwned>(params: Option<Value>) -> Result<T, RpcError> {
    let value = params.unwrap_or_else(|| Value::Object(Default::default()));
    serde_json::from_value(value).map_err(|_| RpcError::new(INVALID_PARAMS, "参数非法"))
}
