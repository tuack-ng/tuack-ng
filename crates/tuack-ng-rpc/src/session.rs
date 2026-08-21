use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Context;
use indexmap::IndexMap;
use serde_json::{Value, json};
use tokio::sync::RwLock;

use tuack_config::Config;
use tuack_config::lang::Language;

use crate::jsonrpc::Event;
use crate::output::write_line;

/// 事件发射器：分配进程级全局单调递增 seq 并立即写 stdout
pub struct EventEmitter {
    seq: AtomicU64,
}

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            seq: AtomicU64::new(0),
        }
    }

    pub fn emit(&self, method: &str, params: Value) {
        let seq = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let mut params = params;
        if let Some(obj) = params.as_object_mut() {
            obj.insert("seq".into(), json!(seq));
        }
        let event = Event::new(method, params);
        write_line(&serde_json::to_string(&event).expect("事件序列化失败"));
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

/// 一个 workspace 的运行时上下文（独立实例，无全局状态）
pub struct RpcContext {
    pub id: String,
    pub cwd: PathBuf,
    pub assets_dirs: Vec<PathBuf>,
    pub languages: IndexMap<String, Language>,
    pub config: RwLock<Option<Config>>,
    /// session-global 配置 revision（乐观并发）
    pub revision: AtomicU64,
    pub emitter: Arc<EventEmitter>,
}

/// 按优先级定位资源目录：开发目录（仅 debug）-> 用户数据目录 -> 系统目录
pub fn assets_dirs() -> anyhow::Result<Vec<PathBuf>> {
    let home = dirs::home_dir().context("无法获取 HOME 环境变量")?;
    let mut dirs = vec![
        #[cfg(debug_assertions)]
        {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("assets")
        },
        dirs::data_local_dir()
            .unwrap_or_else(|| home.join(".local/share"))
            .join("tuack-ng"),
        #[cfg(feature = "nix")]
        {
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent()?.parent()?.join("share/tuack-ng").into())
                .context("找不到资源")?
        },
        #[cfg(not(feature = "nix"))]
        PathBuf::from("/usr/share/tuack-ng/"),
    ];
    dirs.retain(|d| d.exists());
    Ok(dirs)
}

pub fn load_languages(assets_dirs: &[PathBuf]) -> anyhow::Result<IndexMap<String, Language>> {
    let path = assets_dirs
        .iter()
        .find_map(|d| {
            let p = d.join("langs.json");
            p.exists().then_some(p)
        })
        .context("找不到 langs.json")?;
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

/// 从工作目录加载（或发现）工程配置
pub fn load_config(cwd: &std::path::Path) -> anyhow::Result<Option<Config>> {
    let mut loadctx = tuack_config::msgs::LoadContext::new();
    Ok(tuack_config::load_config(&mut loadctx, cwd)?)
}
