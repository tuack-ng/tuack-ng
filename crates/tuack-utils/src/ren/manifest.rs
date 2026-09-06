use crate::prelude::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum TargetType {
    Typst,
    Markdown,
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

fn default_function() -> String {
    "process".to_string()
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
