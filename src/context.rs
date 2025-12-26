use indicatif::MultiProgress;
use std::path::PathBuf;
use std::sync::OnceLock;

pub struct Context {
    pub template_dirs: Vec<PathBuf>,
    pub scaffold_dirs: Vec<PathBuf>,
    pub multiprogress: MultiProgress,
}

static GLOBAL_CONTEXT: OnceLock<Context> = OnceLock::new();

pub fn setup_context(x: Context) -> Result<(), &'static str> {
    GLOBAL_CONTEXT.set(x).map_err(|_| "Already initialized")
}

pub fn get_context() -> &'static Context {
    GLOBAL_CONTEXT.get().expect("Not initialized")
}
