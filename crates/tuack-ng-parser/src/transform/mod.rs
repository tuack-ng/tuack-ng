//! AST 变换工具：就地修改或重建 AST。
//!
//! 参考 markdown-ppp 的 `ast_transform` 常用能力。

use crate::ast::block::BlockKind;
use crate::ast::inline::{Image, InlineKind, Link};
use crate::ast::{Block, Document, Inline, Table, TableCell};

/// 文档级变换 trait。
///
/// 提供常用便捷变换；更复杂的修改可直接重建 `Document`。
pub trait Transform {
    /// 将文档的 blocks 重新组织。
    fn map_blocks<F: FnMut(Block) -> Block>(&mut self, f: F) -> &mut Self;

    /// 变换所有图片的 URL。
    fn transform_image_urls<F: FnMut(&str) -> String>(&mut self, f: F) -> &mut Self;

    /// 变换所有链接的 URL。
    fn transform_link_urls<F: FnMut(&str) -> String>(&mut self, f: F) -> &mut Self;
}

impl Transform for Document {
    fn map_blocks<F: FnMut(Block) -> Block>(&mut self, f: F) -> &mut Self {
        self.blocks = self.blocks.drain(..).map(f).collect();
        self
    }

    fn transform_image_urls<F: FnMut(&str) -> String>(&mut self, mut f: F) -> &mut Self {
        transform_document(
            self,
            |_| {},
            |inline| {
                if let InlineKind::Image(img) = &mut inline.value {
                    transform_image(img, &mut f);
                }
            },
        );
        self
    }

    fn transform_link_urls<F: FnMut(&str) -> String>(&mut self, mut f: F) -> &mut Self {
        transform_document(
            self,
            |_| {},
            |inline| {
                if let InlineKind::Link(link) = &mut inline.value {
                    transform_link(link, &mut f);
                }
            },
        );
        self
    }
}

/// 遍历文档，对每个块/行内节点执行变换。
fn transform_document(
    doc: &mut Document,
    mut on_block: impl FnMut(&mut BlockKind),
    mut on_inline: impl FnMut(&mut Inline),
) {
    fn walk_block(
        block: &mut BlockKind,
        on_block: &mut impl FnMut(&mut BlockKind),
        on_inline: &mut impl FnMut(&mut Inline),
    ) {
        on_block(block);
        match block {
            BlockKind::Paragraph(inlines) => {
                for inline in inlines {
                    walk_inline(inline, on_inline);
                }
            }
            BlockKind::Heading(h) => {
                for inline in &mut h.content {
                    walk_inline(inline, on_inline);
                }
            }
            BlockKind::BlockQuote(blocks) => {
                for b in blocks {
                    walk_block(&mut b.value, on_block, on_inline);
                }
            }
            BlockKind::List(list) => {
                for item in &mut list.items {
                    for b in &mut item.value.blocks {
                        walk_block(&mut b.value, on_block, on_inline);
                    }
                }
            }
            BlockKind::Table(table) => {
                for row in &mut table.rows {
                    for cell in row {
                        for inline in &mut cell.value.content {
                            walk_inline(inline, on_inline);
                        }
                    }
                }
            }
            BlockKind::FootnoteDefinition(fn_def) => {
                for b in &mut fn_def.blocks {
                    walk_block(&mut b.value, on_block, on_inline);
                }
            }
            BlockKind::Container(c) => {
                for b in &mut c.blocks {
                    walk_block(&mut b.value, on_block, on_inline);
                }
            }
            _ => {}
        }
    }

    fn walk_inline(inline: &mut Inline, on_inline: &mut impl FnMut(&mut Inline)) {
        on_inline(inline);
        match &mut inline.value {
            InlineKind::Link(link) => {
                for child in &mut link.children {
                    walk_inline(child, on_inline);
                }
            }
            InlineKind::Emphasis(children)
            | InlineKind::Strong(children)
            | InlineKind::Strikethrough(children) => {
                for child in children {
                    walk_inline(child, on_inline);
                }
            }
            _ => {}
        }
    }

    for block in &mut doc.blocks {
        walk_block(&mut block.value, &mut on_block, &mut on_inline);
    }
}

fn transform_image(img: &mut Image, f: &mut impl FnMut(&str) -> String) {
    img.destination = f(&img.destination);
}

fn transform_link(link: &mut Link, f: &mut impl FnMut(&str) -> String) {
    link.destination = f(&link.destination);
}

#[allow(unused)]
fn _assert_send() {
    fn assert_traits<T: Send>() {}
    assert_traits::<Document>();
    assert_traits::<Block>();
    assert_traits::<Inline>();
    assert_traits::<Table>();
    assert_traits::<TableCell>();
}
