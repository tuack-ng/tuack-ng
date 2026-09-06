use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum TargetType {
    Typst,
    Markdown,
    Extism,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateManifest {
    #[serde(default = "default_use_pretest")]
    pub use_pretest: bool,
    #[serde(default = "default_noi_style")]
    pub noi_style: bool,
    #[serde(default = "default_file_io")]
    pub file_io: bool,
    pub target: TargetType,
    #[serde(default)]
    pub filelist: IndexMap<String, String>,
    #[serde(default)]
    pub processor: Vec<String>,
    #[serde(default)]
    pub extism_plugins: Vec<ExtismProcessorConfig>,
    #[serde(default)]
    pub extism_renderer: Option<ExtismRendererConfig>,
}

/// 一个 extism 处理器插件的配置。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ExtismProcessorConfig {
    /// wasm 插件文件路径（相对模板清单目录）。
    pub wasm: PathBuf,
    /// 插件导出的处理函数名（缺省 `process`）。
    #[serde(default = "default_function")]
    pub function: String,
    /// 是否启用 WASI（默认关闭）。
    #[serde(default)]
    pub with_wasi: bool,
}

/// 一个 extism 渲染器插件的配置。
///
/// 渲染器插件需要写入产物文件，强制启用 WASI。
///
/// 信任边界：渲染器 wasm 可通过 `run_command` 执行宿主命令、经 `get_path`
/// 查询宿主路径，等同在用户机器上运行任意代码。只应加载受信任的模板。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ExtismRendererConfig {
    /// wasm 插件文件路径（相对模板清单目录）。
    pub wasm: PathBuf,
    /// 插件导出的渲染函数名（缺省 `render`）。
    #[serde(default = "default_render_function")]
    pub function: String,
}

fn default_function() -> String {
    "process".to_string()
}

fn default_render_function() -> String {
    "render".to_string()
}

fn default_use_pretest() -> bool {
    false
}

fn default_noi_style() -> bool {
    true
}

fn default_file_io() -> bool {
    true
}
