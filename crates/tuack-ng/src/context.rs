use crate::prelude::*;
use indicatif::MultiProgress;
use std::sync::OnceLock;
use tuack_config::Config;
use tuack_config::lang::Language;
use tuack_config::msgs::LoadContext;

pub struct Context {
    pub assets_dirs: Vec<PathBuf>,
    pub multiprogress: MultiProgress,

    pub config: Option<Config>,
    pub loadctx: LoadContext,
    pub languages: IndexMap<String, Language>,
}

pub static GLOBAL_CONTEXT: OnceLock<Context> = OnceLock::new();

pub fn setup_context(x: Context) -> Result<()> {
    if GLOBAL_CONTEXT.set(x).is_err() {
        bail!("Already initialized");
    }
    Ok(())
}

pub fn gctx() -> &'static Context {
    GLOBAL_CONTEXT.get().expect("Not initialized")
}
