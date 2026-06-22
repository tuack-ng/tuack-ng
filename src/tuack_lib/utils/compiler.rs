use async_trait::async_trait;
use std::time::Duration;

use crate::prelude::*;

/// 运行器元信息。
pub struct RunnerManifest {
    /// 是否支持交互式题目。
    pub interactive: bool,
}

/// 进程资源限制。
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// 时间限制，`None` 表示不限。
    pub time_limit: Option<Duration>,
    /// 内存限制（字节），`None` 表示不限。
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

/// 进程结束状态。
#[derive(Debug)]
#[allow(dead_code)]
pub enum RunStatus {
    Success,
    NonZeroExit(i32),
    TimeLimitExceeded,
    MemoryLimitExceeded,
    InternalError(anyhow::Error),
}

/// `execute()` 的返回结果。
#[derive(Debug)]
pub struct RunResult {
    pub status: RunStatus,
    /// 运行时间，TLE/MLE 时为 `None`。
    pub time: Option<Duration>,
    /// 峰值内存（字节），TLE/MLE 时为 `None`。
    pub memory: Option<u64>,
    /// 程序输出。
    pub output: Vec<u8>,
    /// 程序 stderr。
    pub stderr: Vec<u8>,
}

/// IO 模式。
#[derive(Debug, Clone)]
pub enum IoMode {
    /// 标准 IO
    Stdio,
    /// 文件 IO
    File {
        /// 输入文件名。
        input_name: String,
        /// 输出文件名。
        output_name: String,
    },
}

/// 运行器：编译 + 资源限制执行。
#[allow(unused)]
#[async_trait]
pub trait Runner: Send {
    /// 获取运行器元数据
    fn manifest(&self) -> RunnerManifest;

    /// 做必要的准备工作
    fn prepare(&mut self) -> Result<()>;
    /// 做必要的准备工作（异步）
    async fn prepare_async(&mut self) -> Result<()>;

    /// 设置运行限制
    fn set_limits(&mut self, limits: ResourceLimits);
    /// 设置输入
    fn set_input(&mut self, input: Vec<u8>);
    /// 设置 IO 模式
    fn set_io_mode(&mut self, io_mode: IoMode);
    /// 设置交互
    fn set_interactive(&mut self, grader_file: &PathBuf, header_file: &PathBuf) -> Result<()>;

    /// 清理
    fn cleanup(&mut self) -> Result<()>;

    /// 执行程序，**消耗 `set_limits` 和 `set_input` 设置的值**。
    async fn execute(&mut self) -> Result<RunResult>;
}
