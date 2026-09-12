use std::cmp::max;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::time::{Instant, sleep};

use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::prelude::*;
use tuack_lib::utils::compiler::{ResourceLimits, RunStatus};

/// 监视器专用 runtime：同步代码短时进入异步监督子进程用。
static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// 返回监视器 runtime（首次调用时惰性构建 current_thread runtime）。
pub fn monitor_runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("构建监视 runtime 失败")
    })
}

/// 已 spawn 子进程的 TLE/MLE 监控器。
pub struct ProcessSupervisor {
    limits: ResourceLimits,
}

impl ProcessSupervisor {
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    /// 同步入口：将 `cmd` 作为子进程 spawn，并监督其运行。
    ///
    /// 内部在监视器 runtime 上 `block_on`，短暂进入异步执行 `supervise`。
    pub fn supervise_blocking(
        self,
        cmd: std::process::Command,
    ) -> Result<(RunStatus, Option<Duration>, Option<u64>)> {
        monitor_runtime().block_on(async move {
            let mut child = tokio::process::Command::from(cmd).spawn()?;
            self.supervise(&mut child).await
        })
    }

    /// 监控子进程，返回结束状态、用时和峰值内存。
    pub async fn supervise(
        self,
        child: &mut tokio::process::Child,
    ) -> Result<(RunStatus, Option<Duration>, Option<u64>)> {
        let pid = child.id().context("无法获取进程 PID")?;
        let start = Instant::now();
        let time_limit = self.limits.time_limit.unwrap_or(Duration::MAX);
        let memory_limit = self.limits.memory_limit.unwrap_or(u64::MAX);
        let peak_memory = Arc::new(Mutex::new(0u64));
        let monitoring_peak = Arc::clone(&peak_memory);

        let result = tokio::select! {
            biased;

            _ = async move {
                let mut sys = System::new();
                let sys_pid = Pid::from_u32(pid);
                loop {
                    sleep(Duration::from_millis(20)).await;
                    if start.elapsed() > time_limit.saturating_add(Duration::from_millis(200)) {
                        return;
                    }
                    sys.refresh_processes(ProcessesToUpdate::Some(&[sys_pid]), false);
                    if let Some(process) = sys.process(sys_pid) {
                        let memory = process.memory();
                        let mut peak = monitoring_peak.lock().unwrap();
                        *peak = max(*peak, memory);
                        if memory > memory_limit {
                            return;
                        }
                    } else {
                        return;
                    }
                }
            } => {
                let _ = child.kill().await;
                let final_peak = *peak_memory.lock().unwrap();
                if start.elapsed() > time_limit {
                    info!("进程超时：{}", start.elapsed().as_secs_f64());
                    (RunStatus::TimeLimitExceeded, None, Some(final_peak))
                } else {
                    info!("进程内存超限");
                    (RunStatus::MemoryLimitExceeded, None, Some(final_peak))
                }
            }

            exit_status = child.wait() => {
                let elapsed = start.elapsed();
                let final_peak = *peak_memory.lock().unwrap();

                // 判断是否超时
                if elapsed > time_limit {
                    return Ok((RunStatus::TimeLimitExceeded, None, Some(final_peak)));
                }

                // 判断是否内存超限
                if final_peak > memory_limit {
                    return Ok((RunStatus::MemoryLimitExceeded, None, Some(final_peak)));
                }

                info!("进程结束，耗时：{:?}, 峰值内存：{}, 退出码：{:?}",
                    elapsed, final_peak, exit_status.as_ref().ok().and_then(|s| s.code()));
                match exit_status {
                    Ok(status) if status.success() => {
                        (RunStatus::Success, Some(elapsed), Some(final_peak))
                    }
                    Ok(status) => {
                        (RunStatus::NonZeroExit(status.code().unwrap_or(-1)), Some(elapsed), Some(final_peak))
                    }
                    Err(e) => {
                        (RunStatus::InternalError(e.into()), None, None)
                    }
                }
            }
        };

        Ok(result)
    }
}
