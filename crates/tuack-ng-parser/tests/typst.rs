//! Typst 渲染测试（移植自 markdown-ppp typst_printer/tests，输出格式适配本实现）。
//!
//! 重点验证表格合并（colspan/rowspan）在 Typst 输出中的表现，
//! 以及综合文档完整输出的快照锁定。

mod common;

use tuack_ng_parser::printers::render_typst;

#[test]
fn typst_empty_document() {
    let src = "";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert_eq!(out.trim(), "");
}

#[test]
fn typst_heading() {
    let src = "# 标题";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("#heading(level: 1, [#\"标题\"])"),
        "应输出 Typst 标题，实际：{out}"
    );
}

#[test]
fn typst_table_merged_cells() {
    // 移植自 markdown-ppp test_table_with_merged_cells
    let src = "| A1 | < | A3 |\n| --- | --- | --- |\n| B1 | B2 | ^ |\n";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("table.cell(colspan: 2)"),
        "应含 colspan，实际：{out}"
    );
    assert!(
        out.contains("table.cell(rowspan: 2)"),
        "应含 rowspan，实际：{out}"
    );
    // removed 单元格（`<`/`^`）不应出现在输出中
    assert!(!out.contains("<"), "`<` 格应被跳过，实际：{out}");
}

#[test]
fn typst_plain_table() {
    let src = "| a | b |\n| - | - |\n| c | d |\n";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("#figure(table("),
        "应输出 #figure(table，实际：{out}"
    );
    assert!(out.contains("[#\"a\"]"), "应含单元格 a，实际：{out}");
    assert!(out.contains("[#\"d\"]"), "应含单元格 d，实际：{out}");
}

#[test]
fn typst_emphasis_and_strong() {
    let src = "*em* **st**";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(out.contains("#emph["), "应输出 #emph");
    assert!(out.contains("#strong["), "应输出 #strong");
}

#[test]
fn typst_link() {
    let src = "[text](https://example.com)";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("#link(\"https://example.com\")"),
        "应输出 #link"
    );
}

#[test]
fn typst_image_with_attrs() {
    let src = r#"![foo](/url){width="100%"}"#;
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("#box(image(\"/url\""),
        "应输出 #box(image，实际：{out}"
    );
    assert!(out.contains("alt: \"foo\""), "应含 alt，实际：{out}");
    assert!(out.contains("width: 100%"), "应含 width，实际：{out}");
}

#[test]
fn typst_code_block() {
    let src = "```rust\nfn main() {}\n```";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(out.contains("lang: \"rust\""), "应含 lang");
}

#[test]
fn typst_code_block_crlf() {
    // CRLF 行尾不应残留末尾 `\r`（原 bug：`trim_end_matches('\n')` 留下孤立 `\r`）。
    let src = "```txt\r\nline 1\r\nline 2\r\n```";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("line 1\\r\\nline 2"),
        "应转义内部 CRLF，实际：{out:?}"
    );
    assert!(
        !out.contains("line 2\\r\""),
        "不应残留末尾 \\r，实际：{out:?}"
    );
    assert!(!out.contains("2\\r\""), "不应以 \\r 结尾，实际：{out:?}");
}

#[test]
fn typst_hard_line_break() {
    // 硬换行（行尾两空格）在 typst 应为反斜杠续行，而非折叠成空格
    let src = "第一行  \n第二行";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(out.contains("\\\n"), "硬换行应为反斜杠续行，实际：{out:?}");
}

#[test]
fn typst_soft_line_break() {
    // 软换行（普通换行）→ typst `#linebreak()`（保留换行）
    let src = "第一行\n第二行";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("#\"第一行\"#linebreak()#\"第二行\""),
        "软换行应输出 #linebreak()，实际：{out:?}"
    );
}

#[test]
fn typst_figure_caption() {
    // figure 容器支持 caption 参数
    let src = ":::figure{caption=\"这是标题\"}\n内容\n:::";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(
        out.contains("#figure(caption: [这是标题])"),
        "应输出 #figure(caption:)，实际：{out:?}"
    );
    assert!(out.contains("#par[#\"内容\"]"), "应含内容，实际：{out:?}");
}

#[test]
fn typst_figure_no_caption() {
    // figure 无参数时不输出 ()
    let src = ":::figure\n内容\n:::";
    let out = render_typst(&tuack_ng_parser::parse(src));
    assert!(out.contains("#figure["), "应输出 #figure[，实际：{out:?}");
}

// ---- 综合快照 ----

const COMPREHENSIVE_DOC: &str = include_str!("fixtures/comprehensive.md");

/// 完整 Typst 输出快照（fixture）。
const EXPECTED_TYPST: &str = include_str!("fixtures/comprehensive.typ");

/// 锁定一份覆盖多种语法的完整文档的 Typst 输出。
#[test]
fn typst_comprehensive_snapshot() {
    let doc = tuack_ng_parser::parse(COMPREHENSIVE_DOC);
    let typ = render_typst(&doc);
    assert_eq!(typ, EXPECTED_TYPST, "Typst 输出与快照不一致");
}
