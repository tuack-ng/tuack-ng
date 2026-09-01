use crate::ren::processors::html_table::table_to_html;
use crate::ren::processors::loj::loj_unspan;
use tuack_ng_parser::ast::Document;
use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::block::HeadingKind;
use tuack_ng_parser::ast::block::SetextHeading;

use crate::prelude::*;
pub mod html_table;
pub mod loj;

pub fn process_ast(ast: &mut Document, processors: &Vec<String>) -> Result<Document> {
    for processor in processors {
        match processor.as_str() {
            "loj_table" => {
                for block in &mut ast.blocks {
                    if let BlockKind::Table(table) = &mut block.value {
                        *table = loj_unspan(table)?;
                    }
                }
            }
            "html_table" => {
                let mut blocks = Vec::new();
                for block in &mut ast.blocks {
                    match &block.value {
                        BlockKind::Table(table) => {
                            blocks.push(tuack_ng_parser::span::Spanned::plain(
                                BlockKind::HtmlBlock(table_to_html(table)?),
                            ));
                        }
                        _ => blocks.push(block.clone()),
                    }
                }
                ast.blocks = blocks;
            }
            "uoj_title" => {
                for block in &mut ast.blocks {
                    if let BlockKind::Heading(heading) = &mut block.value {
                        match &mut heading.kind {
                            HeadingKind::Atx(level) => {
                                *level = (*level + 1).min(6);
                            }
                            HeadingKind::Setext(setext_heading) => {
                                heading.kind = match setext_heading {
                                    SetextHeading::Level1 => {
                                        HeadingKind::Setext(SetextHeading::Level2)
                                    }
                                    SetextHeading::Level2 => HeadingKind::Atx(3),
                                };
                            }
                        }
                    }
                }
            }
            processor => bail!("无此处理器：{}", processor),
        }
    }
    Ok(ast.to_owned())
}
