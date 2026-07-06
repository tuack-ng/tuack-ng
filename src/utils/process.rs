use std::cmp::max;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::{Instant, sleep};

use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::prelude::*;
use crate::tuack_lib::utils::compiler::{ResourceLimits, RunStatus};

/// 已 spawn 子进程的 TLE/MLE 监控器。
pub struct ProcessSupervisor {
    limits: ResourceLimits,
}

impl ProcessSupervisor {
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
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
                    info!("进程超时: {}", start.elapsed().as_secs_f64());
                    (RunStatus::TimeLimitExceeded, None, Some(final_peak))
                } else {
                    info!("进程内存超限");
                    (RunStatus::MemoryLimitExceeded, None, Some(final_peak))
                }
            }

            exit_status = child.wait() => {
                let elapsed = start.elapsed();
                let final_peak = *peak_memory.lock().unwrap();
                info!("进程结束，耗时: {:?}, 峰值内存: {}, 退出码: {:?}",
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
