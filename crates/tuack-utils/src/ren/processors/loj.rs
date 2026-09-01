use crate::prelude::*;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::ast::{Table, TableCell, TableCellKind};

pub fn loj_unspan(table: &Table) -> Result<Table> {
    let mut new_table = table.clone();
    for (row_id, row) in table.rows.iter().enumerate() {
        for (col_id, col) in row.iter().enumerate() {
            if col.value.removed_by_extended_table {
                continue;
            }
            if col.value.rowspan.is_none() && col.value.colspan.is_none() {
                // 这是个正常单元格
                let mut new_item = col.value.content.clone();
                new_item.push(tuack_ng_parser::span::Spanned::plain(InlineKind::Html(
                    format!("<!--row:{},col: {}-->", row_id, col_id),
                )));
                *new_table
                    .rows
                    .get_mut(row_id)
                    .unwrap()
                    .get_mut(col_id)
                    .unwrap() = cell_with(new_item);
            } else {
                // 这是个合并单元格
                let mut new_item = col.value.content.clone();
                new_item.push(tuack_ng_parser::span::Spanned::plain(InlineKind::Html(
                    format!("<!--row:{},col: {}-->", row_id, col_id),
                )));
                let rowcnt = col.value.rowspan.unwrap_or(1);
                let colcnt = col.value.colspan.unwrap_or(1);
                for expand_row_id in row_id..(row_id + rowcnt) {
                    for expand_col_id in col_id..(col_id + colcnt) {
                        if !new_table
                            .rows
                            .get_mut(expand_row_id)
                            .unwrap()
                            .get_mut(expand_col_id)
                            .unwrap()
                            .value
                            .removed_by_extended_table
                            && expand_row_id != row_id
                            && expand_col_id != col_id
                        {
                            bail!("表格不满足规范");
                        }
                        *new_table
                            .rows
                            .get_mut(expand_row_id)
                            .unwrap()
                            .get_mut(expand_col_id)
                            .unwrap() = cell_with(new_item.clone());
                    }
                }
            }
        }
    }
    Ok(new_table)
}

fn cell_with(content: Vec<tuack_ng_parser::Inline>) -> TableCell {
    tuack_ng_parser::span::Spanned::plain(TableCellKind {
        content,
        colspan: None,
        rowspan: None,
        removed_by_extended_table: false,
    })
}
