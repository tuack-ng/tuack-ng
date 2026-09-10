//! extism 渲染器插件：宿主侧封装。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use extism::convert::Json;
use extism::{Manifest, Wasm, WasmInput};
use tempfile::TempDir;

use tuack_lib::ren::{RenderDocument, Renderer, RendererOutput};
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;

use crate::prelude::*;

use super::context::{AssetStreams, PluginContext, common_imports, specs_to_outputs};

/// 一个基于 extism 的渲染器插件。
///
/// 插件开启 WASI，宿主把临时目录映射为插件内的 `/`（`/out` 为产物工作区）；
/// 插件返回产物描述列表，资产流由宿主直接取用（host-to-host）。
pub struct ExtismRenderer {
    plugin: Mutex<extism::Plugin>,
    function: String,
    tmp: Arc<TempDir>,
}

impl ExtismRenderer {
    pub fn new(wasm: Vec<u8>, function: String, tmp: Arc<TempDir>) -> Result<Self> {
        let tmp_dir = tmp.path();
        fs::create_dir_all(tmp_dir.join("tmp"))?;
        fs::create_dir_all(tmp_dir.join("out"))?;

        let manifest = Manifest::new([Wasm::data(wasm)])
            .with_allowed_path(tmp_dir.to_string_lossy().to_string(), "/");

        let plugin = extism::Plugin::new(WasmInput::Manifest(manifest), common_imports(), true)
            .map_err(|e| anyhow!(e).context("加载 extism 渲染器失败"))?;

        if !plugin.function_exists(&function) {
            bail!("插件未导出函数：{}", function);
        }

        Ok(Self {
            plugin: Mutex::new(plugin),
            function,
            tmp,
        })
    }
}

impl Renderer for ExtismRenderer {
    fn render(
        &self,
        doc: &RenderDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(PathBuf, Vec<OutputFile>)> {
        let out_dir = self.tmp.path().join("out");

        let streams: AssetStreams = Arc::new(Mutex::new(HashMap::new()));
        let ctx = PluginContext::new(assets, self.tmp.path().to_path_buf(), streams.clone());

        let mut plugin = self
            .plugin
            .lock()
            .map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<RendererOutput> = plugin
            .call_with_host_context(&self.function, Json(doc), ctx)
            .map_err(|e| anyhow!(e).context("调用 extism 渲染器失败"))?;

        let files = specs_to_outputs(output.0.files, &streams, &out_dir, self.tmp.clone())?;

        Ok((output.0.main, files))
    }
}
