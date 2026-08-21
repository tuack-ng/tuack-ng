use std::path::{Path, PathBuf};

/// 配置作用域（强类型；JSON 层为字符串）
#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    Contest,
    Day(String),
    Problem { day: String, problem: String },
}

/// 转义 scope 段内的分隔字符：`/` -> `~1`、`~` -> `~0`（先 `~` 后 `/`）
pub fn escape_segment(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

/// 还原 scope 段：`~1` -> `/`、`~0` -> `~`（先 `~1` 后 `~0`）
pub fn unescape_segment(s: &str) -> String {
    s.replace("~1", "/").replace("~0", "~")
}

impl Scope {
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<String> = s.split('/').map(unescape_segment).collect();
        match parts.as_slice() {
            [day] if day == "contest" => Ok(Scope::Contest),
            [day] if !day.is_empty() => Ok(Scope::Day(day.clone())),
            [day, problem] if !day.is_empty() && !problem.is_empty() => Ok(Scope::Problem {
                day: day.clone(),
                problem: problem.clone(),
            }),
            _ => Err(format!("无效的 scope: {}", s)),
        }
    }
}

/// `file://` URI 转文件系统路径
pub fn uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let raw = if let Some(rest) = uri.strip_prefix("file://") {
        percent_decode(rest)
    } else {
        uri.to_string()
    };
    Ok(PathBuf::from(raw))
}

/// 文件系统路径转 `file://` URI
pub fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.to_string_lossy())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// run 评测目标
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Data,
    Sample,
}

impl Target {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "data" => Ok(Target::Data),
            "sample" => Ok(Target::Sample),
            _ => Err(format!("无效的 target: {}", s)),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Target::Data => "data",
            Target::Sample => "sample",
        }
    }
}

/// 事件构造辅助（seq 由 EventEmitter 注入）
pub mod events {
    use serde_json::{Value, json};

    pub fn run_started(
        session_id: &str,
        run_id: &str,
        problem: &str,
        target: &str,
        tester: &str,
    ) -> Value {
        json!({
            "sessionId": session_id,
            "runId": run_id,
            "problem": problem,
            "target": target,
            "tester": tester,
        })
    }

    pub fn run_output(
        session_id: &str,
        run_id: &str,
        test_id: Option<String>,
        channel: &str,
        text: &str,
    ) -> Value {
        let test_id = test_id.map(Value::String).unwrap_or(Value::Null);
        json!({
            "sessionId": session_id,
            "runId": run_id,
            "testId": test_id,
            "channel": channel,
            "text": text,
        })
    }

    pub fn run_ready(session_id: &str, run_id: &str) -> Value {
        json!({
            "sessionId": session_id,
            "runId": run_id,
        })
    }

    pub fn run_finished(
        session_id: &str,
        run_id: &str,
        state: &str,
        error: Option<String>,
    ) -> Value {
        json!({
            "sessionId": session_id,
            "runId": run_id,
            "state": state,
            "error": error.map(Value::String).unwrap_or(Value::Null),
        })
    }

    pub fn ren_started(session_id: &str, task_id: &str, template: &str, scope: &str) -> Value {
        json!({
            "sessionId": session_id,
            "taskId": task_id,
            "template": template,
            "scope": scope,
        })
    }

    pub fn ren_output(session_id: &str, task_id: &str, channel: &str, text: &str) -> Value {
        json!({
            "sessionId": session_id,
            "taskId": task_id,
            "channel": channel,
            "text": text,
        })
    }

    pub fn ren_progress(
        session_id: &str,
        task_id: &str,
        done: u64,
        total: u64,
        item: &str,
    ) -> Value {
        json!({
            "sessionId": session_id,
            "taskId": task_id,
            "done": done,
            "total": total,
            "item": item,
        })
    }

    pub fn ren_finished(
        session_id: &str,
        task_id: &str,
        status: &str,
        tmp_dir: Option<String>,
        files: Vec<String>,
        warnings: Vec<String>,
        error: Option<String>,
    ) -> Value {
        let files: Vec<Value> = files.iter().map(|f| json!({ "path": f })).collect();
        json!({
            "sessionId": session_id,
            "taskId": task_id,
            "status": status,
            "tmpDir": tmp_dir.map(Value::String).unwrap_or(Value::Null),
            "files": files,
            "warnings": warnings,
            "error": error.map(Value::String).unwrap_or(Value::Null),
        })
    }
}
