//! Visitor 模式：只读遍历 AST。
//!
//! 用法与 markdown-ppp 的 `ast_transform::visitor` 一致：
//!
//! ```rust
//! # use tuack_ng_parser::visitor::{VisitWith, Visitor};
//! # use tuack_ng_parser::ast::{BlockKind, InlineKind};
//! # use tuack_ng_parser::parse;
//! struct Collector { texts: Vec<String> }
//! impl Visitor for Collector {
//!     fn visit_inline(&mut self, inline: &tuack_ng_parser::Inline) {
//!         if let InlineKind::Text(t) = &inline.value {
//!             self.texts.push(t.clone());
//!         }
//!         self.walk_inline(inline);
//!     }
//! }
//! let doc = parse("# Hello\n");
//! let mut v = Collector { texts: Vec::new() };
//! doc.visit_with(&mut v);
//! ```

use crate::ast::block::{BlockKind, Heading};
use crate::ast::inline::{Image, InlineKind, Link};
use crate::ast::list::{List, ListItemKind};
use crate::ast::{Block, Document, Inline, Table, TableCellKind};

/// 只读遍历 AST 的 Visitor。
pub trait Visitor {
    fn visit_document(&mut self, _doc: &Document) {
        self.walk_document(_doc);
    }

    /// 访问块节点（携带 span）。
    fn visit_block(&mut self, _block: &Block) {
        self.walk_block(_block);
    }

    /// 访问行内节点（携带 span）。
    fn visit_inline(&mut self, _inline: &Inline) {
        self.walk_inline(_inline);
    }

    fn visit_heading(&mut self, _heading: &Heading) {
        self.walk_heading(_heading);
    }

    fn visit_list(&mut self, _list: &List) {
        self.walk_list(_list);
    }

    fn visit_list_item(&mut self, _item: &ListItemKind) {
        self.walk_list_item(_item);
    }

    fn visit_table(&mut self, _table: &Table) {
        self.walk_table(_table);
    }

    fn visit_table_cell(&mut self, _cell: &TableCellKind) {
        self.walk_table_cell(_cell);
    }

    fn visit_link(&mut self, _link: &Link) {
        self.walk_link(_link);
    }

    fn visit_image(&mut self, _image: &Image) {
        self.walk_image(_image);
    }

    // ---- 默认遍历 ----

    fn walk_document(&mut self, doc: &Document) {
        for block in &doc.blocks {
            self.visit_block(block);
        }
    }

    fn walk_block(&mut self, block: &Block) {
        match &block.value {
            BlockKind::Paragraph(inlines) => {
                for inline in inlines {
                    self.visit_inline(inline);
                }
            }
            BlockKind::Heading(heading) => self.visit_heading(heading),
            BlockKind::ThematicBreak => {}
            BlockKind::BlockQuote(blocks) => {
                for b in blocks {
                    self.visit_block(b);
                }
            }
            BlockKind::List(list) => self.visit_list(list),
            BlockKind::CodeBlock(_) => {}
            BlockKind::HtmlBlock(_) => {}
            BlockKind::Definition(_) => {}
            BlockKind::Table(table) => self.visit_table(table),
            BlockKind::FootnoteDefinition(fn_def) => {
                for b in &fn_def.blocks {
                    self.visit_block(b);
                }
            }
            BlockKind::Container(c) => {
                for b in &c.blocks {
                    self.visit_block(b);
                }
            }
            BlockKind::LatexBlock(_) => {}
            BlockKind::Empty => {}
        }
    }

    fn walk_inline(&mut self, inline: &Inline) {
        match &inline.value {
            InlineKind::Text(_) => {}
            InlineKind::SoftBreak => {}
            InlineKind::LineBreak => {}
            InlineKind::Code(_) => {}
            InlineKind::Latex(_) => {}
            InlineKind::Html(_) => {}
            InlineKind::Link(link) => self.visit_link(link),
            InlineKind::LinkReference(_) => {}
            InlineKind::Image(img) => self.visit_image(img),
            InlineKind::Emphasis(children) => {
                for c in children {
                    self.visit_inline(c);
                }
            }
            InlineKind::Strong(children) => {
                for c in children {
                    self.visit_inline(c);
                }
            }
            InlineKind::Strikethrough(children) => {
                for c in children {
                    self.visit_inline(c);
                }
            }
            InlineKind::Autolink(_) => {}
            InlineKind::FootnoteReference(_) => {}
            InlineKind::Empty => {}
        }
    }

    fn walk_heading(&mut self, heading: &Heading) {
        for inline in &heading.content {
            self.visit_inline(inline);
        }
    }

    fn walk_list(&mut self, list: &List) {
        for item in &list.items {
            self.visit_list_item(&item.value);
        }
    }

    fn walk_list_item(&mut self, item: &ListItemKind) {
        for b in &item.blocks {
            self.visit_block(b);
        }
    }

    fn walk_table(&mut self, table: &Table) {
        for row in &table.rows {
            for cell in row {
                self.visit_table_cell(&cell.value);
            }
        }
    }

    fn walk_table_cell(&mut self, cell: &TableCellKind) {
        for inline in &cell.content {
            self.visit_inline(inline);
        }
    }

    fn walk_link(&mut self, link: &Link) {
        for child in &link.children {
            self.visit_inline(child);
        }
    }

    fn walk_image(&mut self, _image: &Image) {}
}

/// 在文档上运行 Visitor 的便捷接口。
pub trait VisitWith {
    fn visit_with<V: Visitor>(&self, visitor: &mut V);
}

impl VisitWith for Document {
    fn visit_with<V: Visitor>(&self, visitor: &mut V) {
        visitor.visit_document(self);
    }
}
