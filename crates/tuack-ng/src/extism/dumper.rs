//! extism 导出器插件：宿主侧封装。

use std::sync::Mutex;

use extism::convert::Json;
use extism::{Manifest, Wasm, WasmInput};

use tuack_lib::dump::{DumpDocument, Dumper, DumperOutput};
use tuack_lib::utils::asset::AssetProvider;
use tuack_lib::utils::output::OutputFile;

use crate::prelude::*;

use super::context::{PluginContext, collect_outputs, common_imports};

/// 一个基于 extism 的导出器插件。
///
/// 插件开启 WASI，宿主把临时目录映射为插件内的 `/`（工作与产物）；
/// 插件写文件到 `/out`，宿主扫描后转成产物文件列表，警告随返回值返回。
#[allow(dead_code)] // 预留：Dump 的 extism 目标尚未接入 CLI，待插件包模式引入后启用
pub struct ExtismDumper {
    plugin: Mutex<extism::Plugin>,
    function: String,
    out_dir: PathBuf,
    tmp_dir: PathBuf,
}

#[allow(dead_code)] // 预留：Dump 的 extism 目标尚未接入 CLI，待插件包模式引入后启用
impl ExtismDumper {
    pub fn new(wasm: Vec<u8>, function: String, tmp_dir: PathBuf) -> Result<Self> {
        let plug_out_dir = tmp_dir.join("out");
        fs::create_dir_all(&plug_out_dir)?;
        let plug_tmp_dir = tmp_dir.join("tmp");
        fs::create_dir_all(&plug_tmp_dir)?;

        let manifest = Manifest::new([Wasm::data(wasm)])
            .with_allowed_path(tmp_dir.to_string_lossy().to_string(), "/");

        let plugin = extism::Plugin::new(WasmInput::Manifest(manifest), common_imports(), true)
            .map_err(|e| anyhow!(e).context("加载 extism 导出器失败"))?;

        if !plugin.function_exists(&function) {
            bail!("插件未导出函数：{}", function);
        }

        Ok(Self {
            plugin: Mutex::new(plugin),
            function,
            out_dir: plug_out_dir,
            tmp_dir,
        })
    }
}

impl Dumper for ExtismDumper {
    fn dump(
        &self,
        doc: &DumpDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(Vec<OutputFile>, Vec<String>)> {
        if self.out_dir.exists() {
            fs::remove_dir_all(&self.out_dir)?;
        }
        fs::create_dir_all(&self.out_dir)?;

        let ctx = PluginContext::new(assets, self.tmp_dir.clone());

        let mut plugin = self
            .plugin
            .lock()
            .map_err(|e| anyhow!("插件锁中毒：{}", e))?;
        let output: Json<DumperOutput> = plugin
            .call_with_host_context(&self.function, Json(doc), ctx)
            .map_err(|e| anyhow!(e).context("调用 extism 导出器失败"))?;

        let files = collect_outputs(&self.out_dir)?;

        Ok((files, output.0.warnings))
    }
}
