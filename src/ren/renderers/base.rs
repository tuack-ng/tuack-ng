use crate::config::ContestConfig;
use crate::config::ContestDayConfig;
use crate::config::TemplateManifest;
use crate::ren::RenderQueue;
use anyhow::Result;
use std::path::PathBuf;

pub trait Checker {
    fn check_compiler(&self) -> Result<()>;
    fn new(template_dir: PathBuf) -> Self
    where
        Self: Sized;
}

pub trait Compiler {
    fn compile(&self) -> Result<PathBuf>;
    fn new(
        contest_config: ContestConfig,
        day_config: ContestDayConfig,
        tmp_dir: PathBuf,
        renderqueue: Vec<RenderQueue>,
        manifest: TemplateManifest,
    ) -> Self
    where
        Self: Sized;
}
