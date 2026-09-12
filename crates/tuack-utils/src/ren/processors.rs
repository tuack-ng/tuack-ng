use crate::prelude::*;
use tuack_lib::ren::{ProcessorOutput, RenProcessor};
use tuack_ng_parser::ast::Document;
use tuack_ng_parser::ast::block::{BlockKind, HeadingKind, SetextHeading};
use tuack_ng_parser::span::Spanned;

pub mod extism;
pub mod html_table;
pub mod loj;

/// 内置处理器：`loj_table`。
pub struct LojTableProcessor;

/// 内置处理器：`html_table`。
pub struct HtmlTableProcessor;

/// 内置处理器：`uoj_title`。
pub struct UojTitleProcessor;

impl RenProcessor for LojTableProcessor {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput> {
        let mut ast = doc.clone();
        for block in &mut ast.blocks {
            if let BlockKind::Table(table) = &mut block.value {
                *table = loj::loj_unspan(table)?;
            }
        }
        Ok(ProcessorOutput {
            ast,
            warnings: Vec::new(),
        })
    }
}

impl RenProcessor for HtmlTableProcessor {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput> {
        let mut ast = doc.clone();
        let mut blocks = Vec::new();
        for block in &mut ast.blocks {
            match &block.value {
                BlockKind::Table(table) => {
                    blocks.push(Spanned::plain(BlockKind::HtmlBlock(
                        html_table::table_to_html(table)?,
                    )));
                }
                _ => blocks.push(block.clone()),
            }
        }
        ast.blocks = blocks;
        Ok(ProcessorOutput {
            ast,
            warnings: Vec::new(),
        })
    }
}

impl RenProcessor for UojTitleProcessor {
    fn process(&self, doc: &Document) -> Result<ProcessorOutput> {
        let mut ast = doc.clone();
        for block in &mut ast.blocks {
            if let BlockKind::Heading(heading) = &mut block.value {
                match &mut heading.kind {
                    HeadingKind::Atx(level) => {
                        *level = (*level + 1).min(6);
                    }
                    HeadingKind::Setext(setext_heading) => {
                        heading.kind = match setext_heading {
                            SetextHeading::Level1 => HeadingKind::Setext(SetextHeading::Level2),
                            SetextHeading::Level2 => HeadingKind::Atx(3),
                        };
                    }
                }
            }
        }
        Ok(ProcessorOutput {
            ast,
            warnings: Vec::new(),
        })
    }
}

/// 按名字构造内置处理器；未知名字返回 `None`。
pub fn builtin_processor(name: &str) -> Option<Box<dyn RenProcessor>> {
    match name {
        "loj_table" => Some(Box::new(LojTableProcessor)),
        "html_table" => Some(Box::new(HtmlTableProcessor)),
        "uoj_title" => Some(Box::new(UojTitleProcessor)),
        _ => None,
    }
}
