//! 渲染抽象
//!
//! 渲染指：以数据变换为核心的操作，旨在将原文档转换为各种形式。
//!
//! - `RenderDocument` 是不可变输入，`Renderer::render` 产出 `Vec<OutputFile>`。
//! - 渲染器允许的，可忽略不计的副作用：写自己的临时目录、调用外部命令（如 typst）。
//! - 渲染器禁止：访问 `gctx()`（获取资源可能除外）、直接读取用户资源（除非经 `AssetProvider`）、写最终输出目录。

pub mod document;
pub mod processor;

use crate::utils::output::OutputFile;
pub use document::{
    DateInfo, Problem, ProblemMeta, ProblemType, RenConfig, RenderDocument, SupportLanguage,
};
pub use processor::{ProcessorOutput, RenProcessor};

use crate::prelude::*;
use crate::utils::asset::AssetProvider;

/// 渲染器：`RenderDocument -> (主产物相对路径，产物文件列表)`。
///
/// 资源访问（`AssetProvider`）由调用方注入，所有权移交给渲染器。
pub trait Renderer: Send + Sync {
    fn render(
        &self,
        doc: &RenderDocument,
        assets: Box<dyn AssetProvider>,
    ) -> Result<(PathBuf, Vec<OutputFile>)>;
}

/// 外部命令执行结果（宿主暴露给插件的 `run_command` 返回值）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// 渲染器插件返回：主产物相对路径（用于自动打开）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RendererOutput {
    pub main: PathBuf,
}
