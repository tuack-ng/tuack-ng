use crate::ren::processors::html_table::table_to_html;
use crate::ren::processors::loj::loj_unspan;
use markdown_ppp::ast::Block;
use markdown_ppp::ast::Document;
use markdown_ppp::ast::HeadingKind;
use markdown_ppp::ast::SetextHeading;

use crate::prelude::*;
pub mod html_table;
pub mod loj;

pub fn process_ast(ast: &mut Document, processors: &Vec<String>) -> Result<Document> {
    for processor in processors {
        match processor.as_str() {
            "loj_table" => {
                for block in &mut ast.blocks {
                    match block {
                        Block::Table(table) => {
                            *table = loj_unspan(table)?;
                        }
                        _ => (),
                    }
                }
            }
            "html_table" => {
                let mut blocks = Vec::<Block>::new();
                for block in &mut ast.blocks {
                    match block {
                        Block::Table(table) => blocks.push(Block::HtmlBlock(table_to_html(table)?)),
                        block => blocks.push(block.to_owned()),
                    }
                }
                ast.blocks = blocks;
            }
            "uoj_title" => {
                for block in &mut ast.blocks {
                    match block {
                        Block::Heading(heading) => match &mut heading.kind {
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
                        },
                        _ => (),
                    }
                }
            }
            processor => bail!("无此处理器: {}", processor),
        }
    }
    Ok(ast.to_owned())
}
