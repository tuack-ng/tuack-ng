use crate::config::ContestConfig;
use crate::config::ContestDayConfig;
use crate::ren::RenderQueue;
use std::path::PathBuf;

pub trait Checker {
    fn check_compiler(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn new(template_dir: PathBuf) -> Self
    where
        Self: Sized;
}

pub trait Compiler {
    fn compile(&self) -> Result<PathBuf, Box<dyn std::error::Error>>;
    fn new(
        contest_config: ContestConfig,
        day_config: ContestDayConfig,
        tmp_dir: PathBuf,
        renderqueue: Vec<RenderQueue>,
    ) -> Self
    where
        Self: Sized;
}
