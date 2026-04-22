use std::process::Command as StdCommand;
use tokio::process::Command as TokioCommand;

use crate::prelude::*;

#[allow(unused)]
pub trait Runner {
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
    fn set_std_io(&mut self, input_file: &PathBuf) -> Result<()>; // 参数类型修正
    fn get_output_path(&self) -> Result<PathBuf>;
    fn cleanup(&mut self) -> Result<()>;
}
