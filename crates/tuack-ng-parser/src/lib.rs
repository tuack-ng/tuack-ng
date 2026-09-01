//! tuack-ng 下一代 Markdown 解析器。
//!
//! 基于 [rushdown] 解析，产出自建、可遍历的 AST。

pub mod ast;
pub mod parser;
pub mod printers;
pub mod span;
pub mod transform;
pub mod visitor;

pub use ast::{Block, Document, Inline, TableCell};
pub use parser::parse;
pub use span::{Span, Spanned};
