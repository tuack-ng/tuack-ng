//! 自建 Markdown AST。
//!
//! 结构与 markdown-ppp 的 AST 形状对齐，便于迁移渲染器与 Visitor；
//! 每个节点通过 [`crate::span::Spanned`] 携带可选的源码位置。

pub mod block;
pub mod inline;
pub mod list;
pub mod table;

pub use block::{
    Block, BlockKind, CodeBlock, CodeBlockKind, Container, ContainerParam, FootnoteDefinition,
    Heading, HeadingKind, LinkDefinition, SetextHeading,
};
pub use inline::{
    Autolink, Image, ImageAttributes, Inline, InlineKind, Link, LinkKind, LinkReference,
    LinkReferenceKind,
};
pub use list::{List, ListBulletKind, ListItem, ListItemKind, ListKind};
pub use table::{Alignment, Table, TableCell, TableCellKind};

/// 根节点。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
}

impl Document {
    pub fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}
