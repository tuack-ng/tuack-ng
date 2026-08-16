use crate::config::Config;
use crate::config::lang::Language;
use crate::config::msgs::LoadContext;
use crate::prelude::*;
use indicatif::MultiProgress;
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub enum CurrentLocation {
    /// 不属于任何配置文件
    None,
    /// 配置文件根目录
    Root,
    /// 比赛日配置文件
    Day(String),
    /// 赛题配置文件，
    Problem(String, String),
}

pub struct Context {
    pub assets_dirs: Vec<PathBuf>,
    pub multiprogress: MultiProgress,

    pub config: Option<Config>,
    pub loadctx: LoadContext,
    pub languages: IndexMap<String, Language>,
}

pub static GLOBAL_CONTEXT: RwLock<Option<&'static Context>> = RwLock::new(None);

/// 初始化/重建全局上下文。
///
/// JSON-RPC 模式下每个请求可能切换工作目录并重新加载配置，因此允许
/// 多次调用重建；旧上下文以 `Box::leak` 保活，避免调用方持有的
/// `&'static Context` 悬垂（每次重建只泄漏一个很小的结构）。
pub fn setup_context(x: Context) -> Result<()> {
    let leaked: &'static Context = Box::leak(Box::new(x));
    *GLOBAL_CONTEXT.write().unwrap() = Some(leaked);
    Ok(())
}

pub fn gctx() -> &'static Context {
    try_gctx().expect("Not initialized")
}

pub fn try_gctx() -> Option<&'static Context> {
    *GLOBAL_CONTEXT.read().unwrap()
}
