use crate::prelude::*;
use markdown_ppp::ast::{Inline, Table, TableCell};

pub fn loj_unspan(table: &Table) -> Result<Table> {
    let mut new_table = table.clone();
    for (row_id, row) in table.rows.iter().enumerate() {
        for (col_id, col) in row.iter().enumerate() {
            if col.removed_by_extended_table {
                continue;
            }
            if col.rowspan.is_none() && col.colspan.is_none() {
                // 这是个正常单元格
                let mut new_item = col.content.clone();
                new_item.push(Inline::Html(format!(
                    "<!--row:{},col: {}-->",
                    row_id, col_id
                )));
                *new_table
                    .rows
                    .get_mut(row_id)
                    .unwrap()
                    .get_mut(col_id)
                    .unwrap() = TableCell {
                    content: new_item,
                    colspan: None,
                    rowspan: None,
                    removed_by_extended_table: false,
                };
            } else {
                // 这是个合并单元格
                let mut new_item = col.content.clone();
                new_item.push(Inline::Html(format!(
                    "<!--row:{},col: {}-->",
                    row_id, col_id
                )));
                let rowcnt = col.rowspan.unwrap_or(1);
                let colcnt = col.colspan.unwrap_or(1);
                for expand_row_id in row_id..(row_id + rowcnt) {
                    for expand_col_id in col_id..(col_id + colcnt) {
                        if !new_table
                            .rows
                            .get_mut(expand_row_id)
                            .unwrap()
                            .get_mut(expand_col_id)
                            .unwrap()
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
                            .unwrap() = TableCell {
                            content: new_item.clone(),
                            colspan: None,
                            rowspan: None,
                            removed_by_extended_table: false,
                        };
                    }
                }
            }
        }
    }
    Ok(new_table)
}
