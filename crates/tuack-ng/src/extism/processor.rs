//! extism 处理器插件：宿主侧封装。

use std::sync::Mutex;

use extism::convert::Json;
use extism::WasmInput;
use tuack_lib::ren::{ProcessorOutput, RenProcessor};
use tuack_ng_parser::ast::Document;

use crate::prelude::*;

/// 一个基于 extism 的处理器插件。
pub struct ExtismProcessor {
    plugin: Mutex<extism::Plugin>,
    function: String,
}

impl ExtismProcessor {
    /// 从 WASM 字节构造处理器，并校验导出函数存在。
    pub fn new(wasm: Vec<u8>, function: String, with_wasi: bool) -> Result<Self> {
        let plugin = extism::Plugin::new(
            WasmInput::Data(wasm.into()),
            vec![crate::extism::log_import()],
            with_wasi,
        )
        .map_err(|e| anyhow!(e).context("加载 extism 插件失败"))?;
        if !plugin.function_exists(&function) {
            bail!("插件未导出函数：{}", function);
        }
        Ok(Self {
            plugin: Mutex::new(plugin),
            function,
        })
    }
}

impl RenProcessor for ExtismProcessor {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput> {
        let mut plugin = self.plugin.lock().map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<ProcessorOutput> = plugin
            .call(&self.function, Json(doc))
            .map_err(|e| anyhow!(e).context("调用 extism 插件失败"))?;
        Ok(output.0)
    }
}
