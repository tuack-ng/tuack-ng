//! 表格合并逻辑（移植自 markdown-ppp 的 `process_spans`）。

use crate::ast::inline::InlineKind;
use crate::ast::{Alignment, TableCell, TableCellKind};
use crate::span::Spanned;

/// 处理表格中的跨行/跨列合并标记。
///
/// - 内容恰为 `<` 的单元格：向**左**合并到最近的非合并单元格，其 `colspan` 累加。
/// - 内容恰为 `^` 的单元格：向**上**合并到最近的非合并单元格，其 `rowspan` 累加。
///
/// 被合并的标记单元格标记 `removed_by_extended_table = true`。
pub fn process_spans(rows: &mut [Vec<TableCell>]) {
    // 先按行处理 colspan。
    for row in rows.iter_mut() {
        if row.is_empty() {
            continue;
        }
        for i in 1..row.len() {
            let is_colspan_marker = !row[i].value.removed_by_extended_table
                && row[i].value.content.len() == 1
                && matches!(&row[i].value.content[0].value, InlineKind::Text(t) if t == "<");
            if !is_colspan_marker {
                continue;
            }
            // 向左找最近的正常单元格。
            let mut target_col_idx = i - 1;
            loop {
                if target_col_idx < row.len()
                    && !row[target_col_idx].value.removed_by_extended_table
                {
                    let source_colspan = row[i].value.colspan.unwrap_or(1);
                    let target_colspan = row[target_col_idx].value.colspan.get_or_insert(1);
                    *target_colspan += source_colspan;
                    row[i].value.removed_by_extended_table = true;
                    break;
                }
                if target_col_idx == 0 {
                    break;
                }
                target_col_idx -= 1;
            }
        }
    }

    // 再按列处理 rowspan。
    if rows.len() > 1 && !rows.is_empty() && !rows[0].is_empty() {
        let col_count = rows[0].len();
        for i in 0..col_count {
            for j in 1..rows.len() {
                let is_rowspan_marker = !rows[j][i].value.removed_by_extended_table
                    && rows[j][i].value.content.len() == 1
                    && matches!(&rows[j][i].value.content[0].value, InlineKind::Text(t) if t == "^");
                if !is_rowspan_marker {
                    continue;
                }
                // 向上找最近的正常单元格。
                let mut target_row_idx = j - 1;
                loop {
                    if target_row_idx < rows.len()
                        && i < rows[target_row_idx].len()
                        && !rows[target_row_idx][i].value.removed_by_extended_table
                    {
                        let source_rowspan = rows[j][i].value.rowspan.unwrap_or(1);
                        let source_colspan = rows[j][i].value.colspan;
                        let target_rowspan = rows[target_row_idx][i].value.rowspan.get_or_insert(1);
                        *target_rowspan += source_rowspan;

                        let target_colspan = rows[target_row_idx][i].value.colspan.get_or_insert(1);
                        if let Some(source_colspan) = source_colspan {
                            *target_colspan = (*target_colspan).max(source_colspan);
                        }

                        rows[j][i].value.removed_by_extended_table = true;
                        break;
                    }
                    if target_row_idx == 0 {
                        break;
                    }
                    target_row_idx -= 1;
                }
            }
        }
    }
}

/// 创建空的对齐单元格。
#[allow(dead_code)]
pub(crate) fn blank_cell(span: Option<crate::span::Span>) -> TableCell {
    Spanned {
        value: TableCellKind::new(Vec::new()),
        span,
    }
}

pub(crate) fn alignment_from_rushdown(a: rushdown::ast::TableCellAlignment) -> Alignment {
    match a {
        rushdown::ast::TableCellAlignment::Left => Alignment::Left,
        rushdown::ast::TableCellAlignment::Center => Alignment::Center,
        rushdown::ast::TableCellAlignment::Right => Alignment::Right,
        _ => Alignment::None,
    }
}
