//! 基于 rushdown 的解析器。

pub mod convert;
pub mod ext;
pub mod table;

use rushdown::parser::Parser;

use crate::ast::Document;

/// 解析 Markdown 文本为自建 AST。
pub fn parse(source: &str) -> Document {
    let parser = Parser::with_extensions(
        rushdown::parser::Options::default(),
        ext::default_extensions(),
    );
    let mut reader = rushdown::text::BasicReader::new(source);
    let (arena, doc_ref) = parser.parse(&mut reader);
    convert::convert(&arena, doc_ref, source)
}
