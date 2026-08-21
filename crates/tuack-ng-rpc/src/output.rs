use std::io::{self, Write};
use std::sync::Mutex;

static STDOUT_LOCK: Mutex<()> = Mutex::new(());

/// 行级串行写 stdout（响应与事件共用，保证单行原子性与顺序）
pub fn write_line(s: &str) {
    let _g = STDOUT_LOCK.lock().unwrap();
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{s}");
    let _ = out.flush();
}
