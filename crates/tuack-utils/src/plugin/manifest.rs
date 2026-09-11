//! 插件包清单（`plugin.toml`）的数据结构。

use crate::prelude::*;

/// 插件包清单（`plugin.toml`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// 包名（全局唯一）
    pub name: String,
    /// 插件版本（语义化版本）
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    /// 最低主程序版本，低于则拒绝加载
    #[serde(default)]
    pub minver: Option<String>,
    /// 随包资源目录，映射到插件 WASI 只读路径
    #[serde(default)]
    pub asset_dir: Option<PathBuf>,
    /// wasm 入口文件（相对包目录，全包唯一）；纯模板插件可省略
    #[serde(default)]
    pub entry: Option<PathBuf>,
    /// 发布归档名（供插件市场使用；插件本体不消费）
    #[serde(default)]
    pub artifact_name: Option<String>,
    pub components: Vec<Component>,
}

/// 一个组件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    /// 组件名（同类型内包内唯一）
    pub name: String,
    #[serde(flatten)]
    pub body: ComponentBody,
}

/// 组件体，按 `type` 区分。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ComponentBody {
    Dumper(ExecutableComponent),
    Renderer(ExecutableComponent),
    Processor(ExecutableComponent),
    RenTemplate(RenTemplateComponent),
}

/// 组件类型（命名空间）：不同域允许同名，仅同域内要求唯一。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ComponentKind {
    Dumper,
    Renderer,
    Processor,
    RenTemplate,
}

impl ComponentBody {
    pub(crate) fn kind(&self) -> ComponentKind {
        match self {
            ComponentBody::Dumper(_) => ComponentKind::Dumper,
            ComponentBody::Renderer(_) => ComponentKind::Renderer,
            ComponentBody::Processor(_) => ComponentKind::Processor,
            ComponentBody::RenTemplate(_) => ComponentKind::RenTemplate,
        }
    }
}

/// wasm 可执行组件（dumper / renderer / processor）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutableComponent {
    /// 导出函数名
    pub func: String,
    #[serde(default)]
    pub wasi: bool,
    /// 允许执行的宿主可执行文件白名单
    #[serde(default)]
    pub command: Vec<String>,
}

/// `ren_template` 组件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenTemplateComponent {
    /// 模板目录（包内相对路径）；省略即空目录
    #[serde(default)]
    pub template: Option<PathBuf>,
    /// 渲染器：内置裸名，或本包 renderer 组件名
    pub renderer: String,
    /// 处理器引用列表（内置裸名，或本包 processor 组件名）
    #[serde(default)]
    pub processors: Vec<String>,
    #[serde(default = "default_use_pretest")]
    pub use_pretest: bool,
    #[serde(default = "default_noi_style")]
    pub noi_style: bool,
    #[serde(default = "default_file_io")]
    pub file_io: bool,
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
