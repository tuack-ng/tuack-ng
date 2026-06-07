use async_trait::async_trait;
use std::time::Duration;
use tokio::io::AsyncRead;

use crate::prelude::*;

pub struct RunnerManifest {
    pub interactive: bool,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub time_limit: Option<Duration>,
    pub memory_limit: Option<u64>,
}

impl ResourceLimits {
    pub fn unlimited() -> Self {
        Self {
            time_limit: None,
            memory_limit: None,
        }
    }

    pub fn new(time_limit: Duration, memory_limit: u64) -> Self {
        Self {
            time_limit: Some(time_limit),
            memory_limit: Some(memory_limit),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum RunStatus {
    Success,
    NonZeroExit(i32),
    TimeLimitExceeded,
    MemoryLimitExceeded,
    InternalError(anyhow::Error),
}

#[derive(Debug)]
pub struct RunResult {
    pub status: RunStatus,
    pub time: Option<Duration>,
    pub memory: Option<u64>,
    pub output: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum IoMode {
    Stdio,
    File {
        input_name: String,
        output_name: String,
    },
}

#[allow(unused)]
#[async_trait]
pub trait Runner: Send {
    fn manifest(&self) -> RunnerManifest;

    fn prepare(&mut self) -> Result<()>;
    async fn prepare_async(&mut self) -> Result<()>;

    fn set_limits(&mut self, limits: ResourceLimits);
    fn set_input(&mut self, input: Box<dyn AsyncRead + Send + Unpin>);
    fn set_io_mode(&mut self, io_mode: IoMode);
    fn set_interactive(&mut self, grader_file: &PathBuf, header_file: &PathBuf) -> Result<()>;

    fn cleanup(&mut self) -> Result<()>;

    async fn execute(&mut self) -> Result<RunResult>;
}
