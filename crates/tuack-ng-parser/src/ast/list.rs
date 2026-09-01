//! 列表节点。

use super::block::Block;
use crate::span::Spanned;

/// 列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub kind: ListKind,
    pub items: Vec<ListItem>,
}

/// 列表项节点别名。
pub type ListItem = Spanned<ListItemKind>;

/// 列表种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    /// 有序列表（`1.`、`2.` …），起始编号在渲染时恒为 1。
    Ordered,
    /// 无序列表（`-`、`*`、`+`）。
    Bullet(ListBulletKind),
}

/// 无序列表的项目符号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListBulletKind {
    Dash,
    Star,
    Plus,
}

/// 列表项数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItemKind {
    pub blocks: Vec<Block>,
}

impl ListItemKind {
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { blocks }
    }
}
