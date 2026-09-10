//! extism 处理器插件：宿主侧封装。

use std::sync::Mutex;

use extism::WasmInput;
use extism::convert::Json;
use extism::{Manifest, Wasm};
use tuack_lib::ren::{ProcessorOutput, RenProcessor};
use tuack_ng_parser::ast::Document;

use crate::prelude::*;

/// 一个基于 extism 的处理器插件。
pub struct ExtismProcessor {
    plugin: Mutex<extism::Plugin>,
    function: String,
}

impl ExtismProcessor {
    /// `asset_dir` 为插件随包资源目录，映射到只读 WASI 路径 `/assets`。
    pub fn new(
        wasm: Vec<u8>,
        function: String,
        with_wasi: bool,
        asset_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let mut manifest = Manifest::new([Wasm::data(wasm)]);
        if let Some(asset_dir) = &asset_dir {
            manifest = manifest
                .with_allowed_path(format!("ro:{}", asset_dir.to_string_lossy()), "/assets");
        }
        let plugin = extism::Plugin::new(
            WasmInput::Manifest(manifest),
            vec![crate::plugin::extism::log_import()],
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
        let mut plugin = self
            .plugin
            .lock()
            .map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<ProcessorOutput> = plugin
            .call(&self.function, Json(doc))
            .map_err(|e| anyhow!(e).context("调用 extism 插件失败"))?;
        Ok(output.0)
    }
}
