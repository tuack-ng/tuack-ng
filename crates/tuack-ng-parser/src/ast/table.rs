//! 表格相关节点。

use super::inline::Inline;
use crate::span::Spanned;

/// 表格：行集合 + 列对齐。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// 每行是一组单元格；首行为表头（row 0）。
    pub rows: Vec<Vec<TableCell>>,
    /// 列对齐；`alignments.len() == 列数`。
    pub alignments: Vec<Alignment>,
}

/// 表格单元格节点别名。
pub type TableCell = Spanned<TableCellKind>;

/// 单元格数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableCellKind {
    pub content: Vec<Inline>,
    pub colspan: Option<usize>,
    pub rowspan: Option<usize>,
    pub removed_by_extended_table: bool,
}

impl TableCellKind {
    pub fn new(content: Vec<Inline>) -> Self {
        Self {
            content,
            colspan: None,
            rowspan: None,
            removed_by_extended_table: false,
        }
    }
}

/// 单元格对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Alignment {
    #[default]
    None,
    Left,
    Center,
    Right,
}
