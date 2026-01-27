use crate::config::lang::Language;
use crate::prelude::*;
use indicatif::MultiProgress;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub enum CurrentLocation {
    /// 不属于任何配置文件
    None,
    /// 配置文件根目录
    Root,
    /// 比赛日配置文件
    Day(String),
    /// 赛题配置文件,
    Problem(String, String),
}

pub struct Context {
    pub assets_dirs: Vec<PathBuf>,
    pub multiprogress: MultiProgress,

    pub config: Option<(ContestConfig, CurrentLocation)>,
    pub languages: HashMap<String, Language>,
}

static GLOBAL_CONTEXT: OnceLock<Context> = OnceLock::new();

pub fn setup_context(x: Context) -> Result<()> {
    if GLOBAL_CONTEXT.set(x).is_err() {
        bail!("Already initialized");
    }
    Ok(())
}

pub fn get_context() -> &'static Context {
    GLOBAL_CONTEXT.get().expect("Not initialized")
}
