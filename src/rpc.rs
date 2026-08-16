//! JSON-RPC（stdio）服务。
//!
//! 供 GUI 前端以结构化方式驱动 tuack-ng：进程常驻，请求/响应与通知
//! 均为行分隔的 JSON-RPC 2.0 消息，通过 stdin/stdout 传输。
//!
//! 本模块同时承担「输出改道」职责：RPC 模式下 `msg!`/`emsg!`、log
//! 与进度条绘制统一转为 `output`/`progress` 通知，避免污染 stdout。

use std::io::BufRead as _;
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static RPC_ENABLED: AtomicBool = AtomicBool::new(false);
/// 当前 run 是否保留 ANSI 颜色（由请求参数 color 控制）
static RPC_COLOR: AtomicBool = AtomicBool::new(false);
static RPC_STDOUT: OnceLock<Mutex<std::io::Stdout>> = OnceLock::new();

pub(crate) fn set_color(color: bool) {
    RPC_COLOR.store(color, Ordering::SeqCst);
}

/// 当前 run 是否要求彩色输出（供 supports_color 等询问）
pub fn is_color_enabled() -> bool {
    RPC_COLOR.load(Ordering::SeqCst)
}

/// 进入 RPC 模式：接管 stdout（着色剥离在 emit_* 中进行）。
pub fn enable() {
    RPC_ENABLED.store(true, Ordering::SeqCst);
    let _ = RPC_STDOUT.set(Mutex::new(std::io::stdout()));
}

pub fn is_enabled() -> bool {
    RPC_ENABLED.load(Ordering::SeqCst)
}

fn send(value: &serde_json::Value) {
    if let Some(out) = RPC_STDOUT.get() {
        let mut guard = out.lock().unwrap();
        let _ = serde_json::to_writer(&mut *guard, value);
        let _ = guard.write_all(b"\n");
        let _ = guard.flush();
    }
}

/// 剥离常见 ANSI 转义（颜色/光标），RPC 通知应为纯文本。
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    // CSI：跳过参数字节直到终止字节（@..~）
                    for p in chars.by_ref() {
                        if ('@'..='~').contains(&p) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    // OSC：跳到 BEL 或 ST（ESC \）
                    let mut prev = '\0';
                    for p in chars.by_ref() {
                        if p == '\x07' || (p == '\\' && prev == '\x1b') {
                            break;
                        }
                        prev = p;
                    }
                }
                _ => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 把 `msg!`/`emsg!` 的输出改道为 `output` 通知。
/// 返回 true 表示已作为通知发出（调用方不应再直接打印）。
pub fn emit_output(stream: &str, text: &str) -> bool {
    if !is_enabled() {
        return false;
    }
    send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "output",
        "params": {
            "stream": stream,
            "text": if RPC_COLOR.load(Ordering::SeqCst) { text.to_string() } else { strip_ansi(text) },
        },
    }));
    true
}

/// 发出 `progress` 通知（进度条捕获，见 init.rs 的 RpcTermLike）。
pub fn emit_progress(text: &str) {
    if !is_enabled() {
        return;
    }
    send(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "progress",
        "params": {
            "text": if RPC_COLOR.load(Ordering::SeqCst) { text.to_string() } else { strip_ansi(text) },
        },
    }));
}

// ---------- 请求分发 ----------

/// RPC 的 `run` 方法：语义化命令参数（与 CLI 各 Args 结构体同构）
#[derive(serde::Deserialize)]
#[serde(
    tag = "command",
    rename_all = "snake_case",
    rename_all_fields = "snake_case"
)]
pub enum RpcCommand {
    Ren(crate::ren::RenArgs),
    Gen(crate::generate::GenArgs),
    Test(crate::test::TestArgs),
    Conf(crate::conf::ConfArgs),
    Dmk(crate::dmk::DmkArgs),
    Validate(crate::validate::ValidateArgs),
    Dump(crate::dump::DumpArgs),
    Doc(crate::doc::DocArgs),
}

#[derive(serde::Deserialize)]
struct RunParams {
    cwd: String,
    /// 输出通知是否保留 ANSI 颜色（默认 false，前端若用终端渲染可开启）
    #[serde(default)]
    color: bool,
    #[serde(flatten)]
    command: RpcCommand,
}

fn respond(id: &serde_json::Value, body: serde_json::Value) {
    // 通知型请求（无 id）不回复
    if id.is_null() {
        return;
    }
    let mut msg = serde_json::Map::new();
    msg.insert("jsonrpc".to_string(), "2.0".into());
    msg.insert("id".to_string(), id.clone());
    if let Some(obj) = body.as_object() {
        for (k, v) in obj {
            msg.insert(k.clone(), v.clone());
        }
    }
    send(&serde_json::Value::Object(msg));
}

async fn run_once(multi: &indicatif::MultiProgress, params: RunParams) -> anyhow::Result<()> {
    set_color(params.color);
    std::env::set_current_dir(&params.cwd)?;

    // 与 CLI 初始化相同的跳过/迁移/校验逻辑
    let skip = matches!(
        &params.command,
        RpcCommand::Gen(crate::generate::GenArgs {
            target: crate::generate::Targets::Complete(_)
        })
    );
    let migrating = matches!(
        &params.command,
        RpcCommand::Conf(crate::conf::ConfArgs {
            target: crate::conf::Targets::Migrate
        })
    );
    let validating = matches!(
        &params.command,
        RpcCommand::Doc(crate::doc::DocArgs {
            target: crate::doc::Targets::Validate
        })
    );
    if !skip {
        crate::init::init_context(multi.clone(), migrating, validating)?;
    }

    match params.command {
        RpcCommand::Ren(args) => crate::ren::main(args),
        RpcCommand::Gen(args) => crate::generate::main(args),
        RpcCommand::Test(args) => crate::test::main(args).await,
        RpcCommand::Conf(args) => crate::conf::main(args),
        RpcCommand::Dmk(args) => crate::dmk::main(args).await,
        RpcCommand::Validate(args) => crate::validate::main(args).await,
        RpcCommand::Dump(args) => crate::dump::main(args),
        RpcCommand::Doc(args) => crate::doc::main(args),
    }
}

/// JSON-RPC 主循环：stdin 读行 → 分发 → stdout 回复。
pub async fn main() -> anyhow::Result<()> {
    enable();

    // panic 也改道为通知，避免污染 stdout 流
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_default();
        let _ = emit_output("stderr", &format!("panic: {} @ {location}", info));
        default_hook(info);
    }));

    let multi = crate::init::init_log(&false)?;
    log::info!("rpc booting up");

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<serde_json::Value>(line) else {
            send(
                &serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error"}}),
            );
            continue;
        };
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

        match method {
            "ping" => respond(&id, serde_json::json!({ "result": "pong" })),
            "exit" => {
                respond(&id, serde_json::json!({ "result": null }));
                break;
            }
            "run" => {
                let params: RunParams = match serde_json::from_value(req["params"].clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        respond(
                            &id,
                            serde_json::json!({"error":{"code":-32602,"message":format!("invalid params: {e}")}}),
                        );
                        continue;
                    }
                };
                let started = std::time::Instant::now();
                match run_once(&multi, params).await {
                    Ok(()) => respond(
                        &id,
                        serde_json::json!({"result":{"exit_code":0,"duration_ms":started.elapsed().as_millis()}}),
                    ),
                    Err(e) => respond(
                        &id,
                        serde_json::json!({"error":{"code":-32000,"message":format!("{e:#}")}}),
                    ),
                }
            }
            _ => respond(
                &id,
                serde_json::json!({"error":{"code":-32601,"message":format!("method not found: {method}")}}),
            ),
        }
    }

    Ok(())
}
