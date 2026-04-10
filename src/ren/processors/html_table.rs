use crate::prelude::*;
use html_builder::Html5;
use html_builder::{Buffer, Node};
use markdown_ppp::ast::Alignment;
use markdown_ppp::ast::{Inline, Table, TableCell};
use std::fmt::Write;

/// 将 Table 转换为 HTML 表格字符串
pub fn table_to_html(table: &Table) -> Result<String> {
    if table.rows.is_empty() {
        return Ok(String::new());
    }

    let mut buf = Buffer::new();
    let mut table_node = buf.table();

    let (header_row, body_rows) = table.rows.split_first().unwrap();

    // 构建表头
    let mut thead = table_node.thead();
    build_row(&mut thead.tr(), header_row, true, &table.alignments)?;

    // 构建表体
    let mut tbody = table_node.tbody();
    for row in body_rows {
        build_row(&mut tbody.tr(), row, false, &table.alignments)?;
    }

    Ok(buf.finish())
}

fn build_row(
    tr: &mut Node,
    row: &[TableCell],
    is_header: bool,
    alignments: &[Alignment],
) -> Result<()> {
    for (col_id, col) in row.iter().enumerate() {
        if col.removed_by_extended_table {
            continue;
        }

        let mut cell_node = if is_header { tr.th() } else { tr.td() };

        if let Some(colspan) = col.colspan {
            if colspan > 1 {
                cell_node = cell_node.attr(&format!("colspan=\"{}\"", colspan));
            }
        }

        if let Some(rowspan) = col.rowspan {
            if rowspan > 1 {
                cell_node = cell_node.attr(&format!("rowspan=\"{}\"", rowspan));
            }
        }

        if let Some(align) = alignments.get(col_id) {
            let align_str = match align {
                Alignment::Left => "left",
                Alignment::Center => "center",
                Alignment::Right => "right",
                Alignment::None => "",
            };
            if !align_str.is_empty() {
                cell_node = cell_node.attr(&format!("align=\"{}\"", align_str));
            }
        }

        cell_node = write_inlines(cell_node, &col.content)?;
    }
    Ok(())
}

/// 将 Inline 元素写入到节点中
fn write_inlines<'a>(mut node: Node<'a>, inlines: &[Inline]) -> Result<Node<'a>> {
    for inline in inlines {
        match inline {
            Inline::Text(text) => {
                node.write_str(text)?;
            }
            Inline::LineBreak => {
                node.br();
            }
            Inline::Code(code) => {
                node.code().write_str(code)?;
            }
            Inline::Latex(latex) => {
                node.write_str("$")?;
                node.write_str(latex)?;
                node.write_str("$")?;
            }
            Inline::Html(html) => {
                node = node.raw();
                node.write_str(html)?;
            }
            Inline::Link(link) => {
                let mut a = node.a();
                a = a.attr(&format!("href=\"{}\"", escape_html(&link.destination)));
                if let Some(title) = &link.title {
                    a = a.attr(&format!("title=\"{}\"", escape_html(title)));
                }
                a = write_inlines(a, &link.children)?;
            }
            Inline::LinkReference(_) => {
                msg_warn!("在 Html 中不支持引用式链接");
            }
            Inline::Image(image) => {
                let mut img = node.img();
                img = img.attr(&format!("src=\"{}\"", escape_html(&image.destination)));
                img = img.attr(&format!("alt=\"{}\"", escape_html(&image.alt)));
                if let Some(title) = &image.title {
                    img = img.attr(&format!("title=\"{}\"", escape_html(title)));
                }
                if let Some(attr) = &image.attr {
                    if let Some(width) = &attr.width {
                        img = img.attr(&format!("width=\"{}\"", escape_html(width)));
                    }
                    if let Some(height) = &attr.height {
                        img = img.attr(&format!("height=\"{}\"", escape_html(height)));
                    }
                }
            }
            Inline::Emphasis(content) => {
                let mut em = node.em();
                em = write_inlines(em, content)?;
            }
            Inline::Strong(content) => {
                let mut strong = node.strong();
                strong = write_inlines(strong, content)?;
            }
            Inline::Strikethrough(content) => {
                let mut del = node.del();
                del = write_inlines(del, content)?;
            }
            Inline::Autolink(url) => {
                let mut a = node.a().attr(&format!("href=\"{}\"", escape_html(url)));
                a.write_str(url)?;
            }
            Inline::FootnoteReference(_) => {
                msg_warn!("在 Html 中不支持脚注引用");
            }
            Inline::Empty => {
                // 空元素，什么都不做
            }
        }
    }
    Ok(node)
}

/// 转义 HTML 特殊字符
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
