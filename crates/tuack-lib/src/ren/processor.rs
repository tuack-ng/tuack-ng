//! 渲染处理器契约与插件协议。
//!
//! 处理器在题面解析为 AST 之后、交给渲染器之前运行，对 AST 做变换。
//! 处理器可由内置实现或外部插件（如 WASM）提供；`ProcessorOutput` 是
//! 宿主与插件之间统一的 JSON 协议形状。

use tuack_ng_parser::ast::Document;

use crate::prelude::*;

/// 处理器：对单题 `Document` 做变换，返回变换结果与警告。
pub trait RenProcessor: Send + Sync {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput>;
}

/// 处理器返回的统一协议形状（宿主与插件两端共用，JSON 编解码）。
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorOutput {
    /// 变换后的题目 AST。
    pub ast: Document,
    /// 处理过程中产生的、需展示给用户的警告。
    pub warnings: Vec<String>,
}
