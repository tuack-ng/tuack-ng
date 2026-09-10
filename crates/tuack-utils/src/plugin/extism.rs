//! extism 宿主共享上下文与日志 host 函数（渲染器在 `ren::extism`、导出器在 `dump::extism`、
//! 处理器在 `ren::processors::extism`）。

use extism::UserData;

use crate::prelude::*;

pub mod context;

/// 处理器与渲染器插件共用的日志 host 函数（插件经 `log` 门面转发而来）。
pub(crate) fn log_import() -> extism::Function {
    extism::Function::new(
        "plugin_log",
        [extism::PTR, extism::PTR],
        [],
        UserData::default(),
        plugin_log,
    )
    .with_namespace(extism::EXTISM_USER_MODULE)
}

extism::host_fn!(plugin_log(level: i32, msg: String) {
    match level {
        4 => log::error!("[plugin] {msg}"),
        3 => log::warn!("[plugin] {msg}"),
        2 => log::info!("[plugin] {msg}"),
        1 => log::debug!("[plugin] {msg}"),
        _ => log::trace!("[plugin] {msg}"),
    }
    Ok(())
});
