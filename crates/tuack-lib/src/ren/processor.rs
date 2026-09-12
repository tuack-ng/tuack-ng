//! 渲染处理器契约。
//!
//! 处理器对单题 AST 做变换，返回变换结果与警告。

use tuack_ng_parser::ast::Document;

use crate::prelude::*;

/// 处理器：对单题 `Document` 做变换，返回变换结果与警告。
pub trait RenProcessor: Send + Sync {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput>;
}

/// 处理器返回值。
#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessorOutput {
    /// 变换后的题目 AST。
    pub ast: Document,
    /// 处理过程中产生的、需展示给用户的警告。
    pub warnings: Vec<String>,
}
