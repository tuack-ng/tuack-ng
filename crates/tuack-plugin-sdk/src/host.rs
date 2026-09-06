//! 宿主函数导入与封装。

use std::io::Read;

use extism_pdk::{Error, Json, host_fn};
use tuack_lib::ren::CommandResult;

/// 宿主提供的 host 函数（由 extism 的 `host_context` 承载状态）。
#[host_fn("extism:host/user")]
unsafe extern "ExtismHost" {
    pub fn asset_open(problem_idx: u64, url: String) -> u64;
    pub fn asset_read(asset_id: u64, len: u64) -> Vec<u8>;
    pub fn asset_close(asset_id: u64);
    pub fn asset_copy(asset_id: u64, dest: String);
    pub fn run_command(args: Json<Vec<String>>, cwd: Json<String>) -> Json<CommandResult>;
    pub fn plugin_log(level: i32, msg: String);
    pub fn host_get_path(path: String) -> String;
}

/// 资产流：把宿主的 `asset_open/read/close` 封装为 `Read` 对象。
///
/// 注意：此流与宿主持有的流共享当前位置。若先 `read` 到中间或末尾，再把本句柄
/// 放进 `OutputFile` 返回，落盘时 `copy_to_host` 会从当前位置继续拷贝，产物将只
/// 含剩余部分（甚至为空）。要整份拷贝，请勿提前 `read`。
pub struct AssetReader {
    id: u64,
}

impl AssetReader {
    /// 打开第 `problem_idx` 题的资产 `url`，返回可读流。
    pub fn open(problem_idx: u64, url: &str) -> Result<Self, Error> {
        let id = unsafe { asset_open(problem_idx, url.to_string()) }?;
        Ok(Self { id })
    }

    /// 请求宿主把资产流从当前位置直接拷贝到目标 WASI 路径，避免跨 wasm 逐块读取。
    pub fn copy_to_host(&self, dest: &str) -> Result<(), Error> {
        unsafe { asset_copy(self.id, dest.to_string()) }
    }
}

impl Read for AssetReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let data = unsafe { asset_read(self.id, buf.len() as u64) }
            .map_err(|e| std::io::Error::other(format!("{e:?}")))?;
        if data.is_empty() {
            return Ok(0);
        }
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data);
        Ok(n)
    }
}

impl Drop for AssetReader {
    fn drop(&mut self) {
        unsafe {
            let _ = asset_close(self.id);
        }
    }
}

/// 执行外部命令（宿主侧执行，插件拿回退出码与输出）。
pub fn command(args: &[&str], cwd: &str) -> Result<CommandResult, Error> {
    let args = Json(args.iter().map(|s| s.to_string()).collect::<Vec<String>>());
    let cwd = Json(cwd.to_string());
    let result = unsafe { run_command(args, cwd) }?;
    Ok(result.0)
}

/// 获取 WASI 路径对应的宿主真实文件系统路径（如 `/out` -> 宿主产物目录）。
pub fn get_path(path: &str) -> Result<String, Error> {
    let resolved = unsafe { host_get_path(path.to_string()) }?;
    Ok(resolved)
}
