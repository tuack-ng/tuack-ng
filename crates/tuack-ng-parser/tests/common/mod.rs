//! 测试辅助：从 markdown-ppp 迁移的测试共用的构造与断言工具。

#![allow(dead_code)]

use tuack_ng_parser::ast::block::{
    BlockKind, CodeBlock, CodeBlockKind, Heading, HeadingKind, SetextHeading,
};
use tuack_ng_parser::ast::inline::{Image, ImageAttributes, InlineKind, Link};
use tuack_ng_parser::ast::list::{List, ListBulletKind, ListItemKind, ListKind};
use tuack_ng_parser::ast::{Alignment, Block, Document, Table, TableCell, TableCellKind};
use tuack_ng_parser::span::Spanned;

/// 无 span 的块。
pub fn b(kind: BlockKind) -> Block {
    Spanned::plain(kind)
}

/// 无 span 的行内。
pub fn i(kind: InlineKind) -> tuack_ng_parser::Inline {
    Spanned::plain(kind)
}

/// 无 span 的表格单元格。
pub fn cell(content: Vec<tuack_ng_parser::Inline>) -> TableCell {
    Spanned::plain(TableCellKind::new(content))
}

/// 带合并信息的表格单元格。
pub fn cell_with(
    content: Vec<tuack_ng_parser::Inline>,
    colspan: Option<usize>,
    rowspan: Option<usize>,
    removed: bool,
) -> TableCell {
    Spanned::plain(TableCellKind {
        content,
        colspan,
        rowspan,
        removed_by_extended_table: removed,
    })
}

/// 无 span 的列表项。
pub fn li(blocks: Vec<Block>) -> Spanned<ListItemKind> {
    Spanned::plain(ListItemKind::new(blocks))
}

/// 断言解析结果与期望块集合一致（忽略 span，只比结构）。
pub fn assert_blocks(source: &str, expected: Vec<Block>) {
    let doc = tuack_ng_parser::parse(source);
    let mut actual = doc.blocks;
    strip_spans(&mut actual);
    assert_eq!(actual, expected, "source: {source:?}");
}

/// 递归剥掉所有 span（公开，供往返测试使用）。
pub fn strip_public_spans(blocks: &mut Vec<Block>) {
    strip_spans(blocks);
}

/// 递归剥掉所有 span，便于只比较结构。
fn strip_spans(blocks: &mut Vec<Block>) {
    for block in blocks {
        block.span = None;
        strip_block_kind(&mut block.value);
    }
}

fn strip_block_kind(kind: &mut BlockKind) {
    match kind {
        BlockKind::Paragraph(inlines) => strip_inlines(inlines),
        BlockKind::Heading(h) => strip_inlines(&mut h.content),
        BlockKind::BlockQuote(blocks) => strip_spans(blocks),
        BlockKind::List(list) => {
            for item in &mut list.items {
                item.span = None;
                strip_spans(&mut item.value.blocks);
            }
        }
        BlockKind::Table(table) => {
            for row in &mut table.rows {
                for cell in row {
                    cell.span = None;
                    strip_inlines(&mut cell.value.content);
                }
            }
        }
        BlockKind::FootnoteDefinition(fn_def) => strip_spans(&mut fn_def.blocks),
        BlockKind::Container(c) => strip_spans(&mut c.blocks),
        _ => {}
    }
}

fn strip_inlines(inlines: &mut Vec<tuack_ng_parser::Inline>) {
    for inline in inlines {
        inline.span = None;
        match &mut inline.value {
            InlineKind::Link(link) => strip_inlines(&mut link.children),
            InlineKind::Emphasis(c) | InlineKind::Strong(c) | InlineKind::Strikethrough(c) => {
                strip_inlines(c)
            }
            _ => {}
        }
    }
}

/// 断言解析结果与期望块集合一致（含 Document 整体断言）。
pub fn assert_document(source: &str, expected: Document) {
    let doc = tuack_ng_parser::parse(source);
    assert_eq!(doc, expected, "source: {source:?}");
}

// 便捷构造 —— 文本/代码/强调等（减少样板）。
pub fn text(s: &str) -> tuack_ng_parser::Inline {
    i(InlineKind::Text(s.to_string()))
}

pub fn code(s: &str) -> tuack_ng_parser::Inline {
    i(InlineKind::Code(s.to_string()))
}

pub fn soft_break() -> tuack_ng_parser::Inline {
    i(InlineKind::SoftBreak)
}

pub fn emphasis(children: Vec<tuack_ng_parser::Inline>) -> tuack_ng_parser::Inline {
    i(InlineKind::Emphasis(children))
}

pub fn strong(children: Vec<tuack_ng_parser::Inline>) -> tuack_ng_parser::Inline {
    i(InlineKind::Strong(children))
}

pub fn strikethrough(children: Vec<tuack_ng_parser::Inline>) -> tuack_ng_parser::Inline {
    i(InlineKind::Strikethrough(children))
}

pub fn link(
    destination: &str,
    title: Option<&str>,
    children: Vec<tuack_ng_parser::Inline>,
) -> tuack_ng_parser::Inline {
    i(InlineKind::Link(Link {
        destination: destination.to_string(),
        title: title.map(|t| t.to_string()),
        children,
    }))
}

pub fn image(
    destination: &str,
    title: Option<&str>,
    alt: &str,
    attr: Option<ImageAttributes>,
) -> tuack_ng_parser::Inline {
    i(InlineKind::Image(Image {
        destination: destination.to_string(),
        title: title.map(|t| t.to_string()),
        alt: alt.to_string(),
        attr,
    }))
}

pub fn attr(width: Option<&str>, height: Option<&str>) -> ImageAttributes {
    ImageAttributes {
        width: width.map(|v| v.to_string()),
        height: height.map(|v| v.to_string()),
    }
}

pub fn para(inlines: Vec<tuack_ng_parser::Inline>) -> BlockKind {
    BlockKind::Paragraph(inlines)
}

pub fn heading_atx(level: u8, content: Vec<tuack_ng_parser::Inline>) -> BlockKind {
    BlockKind::Heading(Heading {
        kind: HeadingKind::Atx(level),
        content,
    })
}

pub fn setext_heading(level: u8, content: Vec<tuack_ng_parser::Inline>) -> BlockKind {
    let kind = if level == 1 {
        HeadingKind::Setext(SetextHeading::Level1)
    } else {
        HeadingKind::Setext(SetextHeading::Level2)
    };
    BlockKind::Heading(Heading { kind, content })
}

pub fn code_block(info: Option<&str>, literal: &str) -> BlockKind {
    BlockKind::CodeBlock(CodeBlock {
        kind: CodeBlockKind::Fenced {
            info: info.map(|s| s.to_string()),
        },
        literal: literal.to_string(),
    })
}

pub fn indented_code(literal: &str) -> BlockKind {
    BlockKind::CodeBlock(CodeBlock {
        kind: CodeBlockKind::Indented,
        literal: literal.to_string(),
    })
}

pub fn bullet_list(items: Vec<Spanned<ListItemKind>>) -> BlockKind {
    BlockKind::List(List {
        kind: ListKind::Bullet(ListBulletKind::Dash),
        items,
    })
}

pub fn ordered_list(items: Vec<Spanned<ListItemKind>>) -> BlockKind {
    BlockKind::List(List {
        kind: ListKind::Ordered,
        items,
    })
}

pub fn table(rows: Vec<Vec<TableCell>>, alignments: Vec<Alignment>) -> BlockKind {
    BlockKind::Table(Table { rows, alignments })
}

pub fn blockquote(blocks: Vec<Block>) -> BlockKind {
    BlockKind::BlockQuote(blocks)
}
