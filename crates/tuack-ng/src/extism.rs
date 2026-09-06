//! extism 宿主封装：处理器插件、渲染器插件与导出器插件。

use extism::UserData;

use crate::prelude::*;

pub mod context;
pub mod dumper;
pub mod processor;
pub mod renderer;

// ExtismDumper 预留待插件包模式接入，暂未在 CLI 引用。
#[allow(unused_imports)]
pub use dumper::ExtismDumper;
pub use processor::ExtismProcessor;
pub use renderer::ExtismRenderer;

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
