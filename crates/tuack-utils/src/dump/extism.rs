//! extism 导出器插件：宿主侧封装。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use extism::convert::Json;
use extism::{Manifest, Wasm, WasmInput};
use tempfile::TempDir;

use tuack_lib::dump::{DumpDocument, Dumper, DumperOutput};
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;

use crate::prelude::*;

use crate::plugin::extism::context::{
    AssetStreams, PluginContext, common_imports, specs_to_outputs,
};

/// 一个基于 extism 的导出器插件。
///
/// 插件开启 WASI，宿主把临时目录映射为插件内的 `/`（`/out` 为产物工作区）；
/// 插件返回产物描述列表与警告，资产流由宿主直接取用（host-to-host）。
pub struct ExtismDumper {
    plugin: Mutex<extism::Plugin>,
    function: String,
    tmp: Arc<TempDir>,
    command: Vec<String>,
}

impl ExtismDumper {
    /// `asset_dir` 为插件随包资源目录，映射到只读 WASI 路径 `/assets`；
    /// `command` 为允许执行的宿主命令白名单。
    pub fn new(
        wasm: Vec<u8>,
        function: String,
        tmp: Arc<TempDir>,
        asset_dir: Option<PathBuf>,
        command: Vec<String>,
    ) -> Result<Self> {
        let tmp_dir = tmp.path();
        fs::create_dir_all(tmp_dir.join("tmp"))?;
        fs::create_dir_all(tmp_dir.join("out"))?;

        let mut manifest = Manifest::new([Wasm::data(wasm)])
            .with_allowed_path(tmp_dir.to_string_lossy().to_string(), "/");
        if let Some(asset_dir) = &asset_dir {
            manifest = manifest
                .with_allowed_path(format!("ro:{}", asset_dir.to_string_lossy()), "/assets");
        }

        let plugin = extism::Plugin::new(WasmInput::Manifest(manifest), common_imports(), true)
            .map_err(|e| anyhow!(e).context("加载 extism 导出器失败"))?;

        if !plugin.function_exists(&function) {
            bail!("插件未导出函数：{}", function);
        }

        Ok(Self {
            plugin: Mutex::new(plugin),
            function,
            tmp,
            command,
        })
    }
}

impl Dumper for ExtismDumper {
    fn dump(
        &self,
        doc: &DumpDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(Vec<OutputFile>, Vec<String>)> {
        let out_dir = self.tmp.path().join("out");

        let streams: AssetStreams = Arc::new(Mutex::new(HashMap::new()));
        let ctx = PluginContext::new(
            assets,
            self.tmp.path().to_path_buf(),
            streams.clone(),
            self.command.clone(),
        );

        let mut plugin = self
            .plugin
            .lock()
            .map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<DumperOutput> = plugin
            .call_with_host_context(&self.function, Json(doc), ctx)
            .map_err(|e| anyhow!(e).context("调用 extism 导出器失败"))?;

        let files = specs_to_outputs(output.0.files, &streams, &out_dir, self.tmp.clone())?;

        Ok((files, output.0.warnings))
    }
}
