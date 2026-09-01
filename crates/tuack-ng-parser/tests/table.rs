//! 表格测试：基础结构、合并单元格（colspan/rowspan/removed）、复杂合并、
//! 对齐、转义 pipe、多列截断、CJK 显示宽度对齐。

mod common;

use tuack_ng_parser::ast::{Alignment, BlockKind};

#[test]
fn table_basic() {
    let src = "| foo | bar |\n| --- | --- |\n| baz | bim |";
    let doc = tuack_ng_parser::parse(src);
    let table = match &doc.blocks[0].value {
        BlockKind::Table(t) => t,
        _ => panic!("应为表格"),
    };
    assert_eq!(table.rows.len(), 2);
    assert_eq!(table.alignments.len(), 2);
    assert_eq!(table.rows[0][0].value.content.len(), 1);
}

#[test]
fn table_with_merged_cells() {
    // 移植自 markdown-ppp table.rs: table_with_merged_cells
    let src = "| A1 | < | A3 |\n| --- | --- | --- |\n| B1 | B2 | ^ |";
    let doc = tuack_ng_parser::parse(src);
    let table = match &doc.blocks[0].value {
        BlockKind::Table(t) => t,
        _ => panic!("应为表格"),
    };
    let header = &table.rows[0];
    // A1 colspan=2
    assert_eq!(header[0].value.colspan, Some(2));
    // `<` 格被标记 removed
    assert!(header[1].value.removed_by_extended_table);
    // A3 rowspan=2
    assert_eq!(header[2].value.rowspan, Some(2));
    // B 行末 `^` 格被标记 removed
    assert!(table.rows[1][2].value.removed_by_extended_table);
}

#[test]
fn table_with_complex_spans() {
    // 移植自 markdown-ppp table.rs: table_with_complex_spans
    let src = "| A | B | C |\n|:-:|:-:|:-:|\n| D | < | E |\n| ^ | < | F |";
    let doc = tuack_ng_parser::parse(src);
    let table = match &doc.blocks[0].value {
        BlockKind::Table(t) => t,
        _ => panic!("应为表格"),
    };
    // D 同时 colspan=2 且 rowspan=2
    let d = &table.rows[1][0];
    assert_eq!(d.value.colspan, Some(2), "D 应 colspan=2");
    assert_eq!(d.value.rowspan, Some(2), "D 应 rowspan=2");
    // 各合并标记格被移除
    assert!(
        table.rows[1][1].value.removed_by_extended_table,
        "row1 col1 (<) 应被移除"
    );
    assert!(
        table.rows[2][0].value.removed_by_extended_table,
        "row2 col0 (^) 应被移除"
    );
    assert!(
        table.rows[2][1].value.removed_by_extended_table,
        "row2 col1 (<) 应被移除"
    );
}

#[test]
fn table_alignment() {
    // :-- 左对齐，--: 右对齐，:-: 居中
    let src = "| foo | bar | baz |\n| :-- | --: | :-: |\n| a | b | c |";
    let doc = tuack_ng_parser::parse(src);
    let table = match &doc.blocks[0].value {
        BlockKind::Table(t) => t,
        _ => panic!("应为表格"),
    };
    assert_eq!(table.alignments[0], Alignment::Left);
    assert_eq!(table.alignments[1], Alignment::Right);
    assert_eq!(table.alignments[2], Alignment::Center);
}

#[test]
fn table_escaped_pipe() {
    // 转义 | 在单元格内
    let src = "| foo | bar |\n| --- | --- |\n| baz | b\\|im |";
    let doc = tuack_ng_parser::parse(src);
    let table = match &doc.blocks[0].value {
        BlockKind::Table(t) => t,
        _ => panic!("应为表格"),
    };
    // 第二行第二列内容含 |（转义后）
    let cell_text: String = table.rows[1][1]
        .value
        .content
        .iter()
        .map(|c| match &c.value {
            tuack_ng_parser::ast::InlineKind::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect();
    assert!(cell_text.contains('|'), "应含 |，实际 {cell_text:?}");
}

#[test]
fn table_truncate_extra_columns() {
    // 数据行多于列数时截断
    let src = "| header1 | header2 |\n| ------- | ------- |\n| cell1 | cell2 | extra1 | extra2 |";
    let doc = tuack_ng_parser::parse(src);
    let table = match &doc.blocks[0].value {
        BlockKind::Table(t) => t,
        _ => panic!("应为表格"),
    };
    for row in &table.rows {
        assert!(row.len() <= 2, "应截断到 2 列");
    }
}

#[test]
fn table_cjk_alignment() {
    // 含中文/全角字符的表格，对齐应按显示宽度（中文占 2 格）。
    let src = "| 测试点编号 | $n$ | 特殊性质 |\n| :-: | :-: | :-: |\n| $1$ | $2$ | 无 |";
    let doc = tuack_ng_parser::parse(src);
    let md = tuack_ng_parser::printers::render_markdown(&doc);
    let lines: Vec<&str> = md.trim().lines().collect();
    assert_eq!(lines.len(), 3, "应 3 行表格，实际：{md}");

    // 每行应有相同的 `|` 数量（结构完整）。
    let pipe_counts: Vec<usize> = lines.iter().map(|l| l.matches('|').count()).collect();
    for count in &pipe_counts {
        assert_eq!(count, &pipe_counts[0], "表格 `|` 数量不一致，行：{md}");
    }
}
