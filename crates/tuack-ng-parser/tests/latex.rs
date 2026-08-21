//! LaTeX 公式测试：行内 `$...$`、块级 `$$...$$`（跨行）、同行 `$$..$$` 不解析为块、
//! markdown/typst 渲染与 roundtrip。

mod common;

use common::*;
use tuack_ng_parser::ast::{BlockKind, InlineKind};

#[test]
fn latex_inline() {
    // `$...$` → 行内 Latex
    assert_blocks(
        "公式 $x^2$ 内联",
        vec![b(para(vec![
            text("公式 "),
            i(InlineKind::Latex("x^2".to_string())),
            text(" 内联"),
        ]))],
    );
}

#[test]
fn latex_inline_multiple() {
    let doc = tuack_ng_parser::parse("a $x$ b");
    match &doc.blocks[0].value {
        BlockKind::Paragraph(inlines) => {
            assert!(
                inlines
                    .iter()
                    .any(|i| matches!(&i.value, InlineKind::Latex(c) if c == "x"))
            );
        }
        _ => panic!("应为段落"),
    }
}

#[test]
fn latex_display_block() {
    // 跨行 `$$\n...\n$$` → LatexBlock（行间）
    let doc = tuack_ng_parser::parse("$$\nE = mc^2\n$$\n");
    match &doc.blocks[0].value {
        BlockKind::LatexBlock(content) => assert_eq!(content, "E = mc^2\n"),
        other => panic!("应为 LatexBlock，实际 {other:?}"),
    }
}

#[test]
fn latex_same_line_dollar_is_not_block() {
    // 同行 `$$...$$` 不识别为 LatexBlock（保持为文本）
    let doc = tuack_ng_parser::parse("$$E = mc^2$$");
    assert!(
        !doc.blocks
            .iter()
            .any(|b| matches!(&b.value, BlockKind::LatexBlock(_))),
        "同行 $$..$$ 不应是 LatexBlock"
    );
}

#[test]
fn latex_display_roundtrip() {
    let src = "$$\nE = mc^2\n$$\n";
    let doc = tuack_ng_parser::parse(src);
    let md = tuack_ng_parser::printers::render_markdown(&doc);
    assert!(md.contains("$$"), "MD 应含 $$..$$，实际：{md}");
}

#[test]
fn latex_typst_output() {
    let src = "$$\nE = mc^2\n$$\n";
    let doc = tuack_ng_parser::parse(src);
    let typ = tuack_ng_parser::printers::render_typst(&doc);
    assert!(
        typ.contains("#mi(block: true"),
        "typst 应输出 #mi(block: true)，实际：{typ}"
    );
    assert!(typ.contains("E = mc^2"));
}

#[test]
fn latex_display_multiline() {
    // 跨行 $$...$$ 行间公式
    let src = "$$\n\\gcd\\left(\\binom{n}{1},\\binom{n}{m}\\right)\n$$\n";
    let doc = tuack_ng_parser::parse(src);
    match &doc.blocks[0].value {
        BlockKind::LatexBlock(content) => {
            assert!(
                content.contains("\\gcd"),
                "应包含公式内容，实际 {content:?}"
            );
            assert!(
                content.contains("\\binom"),
                "应包含 binom，实际 {content:?}"
            );
        }
        other => panic!("应为 LatexBlock，实际 {other:?}"),
    }
}

#[test]
fn latex_display_multiline_roundtrip() {
    // 用户失败示例
    let src = "给定 $n,m$，求出：\n\n$$\n\\gcd\\left(\\binom{n}{1},\\binom{n}{2},\\cdots,\\binom{n}{m-1},\\binom{n}{m}\\right)=\\gcd_{i=1}^m\\binom{n}{i}\n$$\n\n的值。";
    let doc = tuack_ng_parser::parse(src);
    // 找 LatexBlock
    let has_block = doc
        .blocks
        .iter()
        .any(|b| matches!(&b.value, BlockKind::LatexBlock(_)));
    assert!(has_block, "应识别出 LatexBlock，实际块：{:#?}", doc.blocks);
    let md = tuack_ng_parser::printers::render_markdown(&doc);
    assert!(md.contains("$$"), "MD 应含 $$..$$，实际：{md}");
}
