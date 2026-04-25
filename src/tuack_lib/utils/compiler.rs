use async_trait::async_trait;
use std::process::Command as StdCommand;
use tokio::process::Command as TokioCommand;

use crate::prelude::*;

pub struct RunnerManifest {
    pub interactive: bool,
}

#[allow(unused)]
#[async_trait]
pub trait Runner: Send {
    fn manifest(&self) -> RunnerManifest;

    fn prepare(&mut self) -> Result<()>;
    async fn prepare_async(&mut self) -> Result<()>;
    fn get_run(&mut self) -> Result<StdCommand>;
    async fn get_run_async(&mut self) -> Result<TokioCommand>;
    fn set_file_io(
        &mut self,
        input_file: &PathBuf,
        input_name: &String,
        output_name: &String,
    ) -> Result<()>;
    fn set_std_io(&mut self, input_file: &PathBuf) -> Result<()>;
    fn set_interactive(&mut self, grader_file: &PathBuf, header_file: &PathBuf) -> Result<()>;
    fn get_output_path(&self) -> Result<PathBuf>;
    fn cleanup(&mut self) -> Result<()>;
}
