//! 冒烟测试：核心链路端到端——解析后渲染 Markdown 与 Typst 均正常。

use tuack_ng_parser::parse;
use tuack_ng_parser::printers::{render_markdown, render_typst};

#[test]
fn render_roundtrip() {
    let src = "# 标题\n\n正文 *强调*。\n";
    let doc = parse(src);
    let md = render_markdown(&doc);
    assert!(md.contains("# 标题"), "MD 应含标题，实际：{md}");
    let typ = render_typst(&doc);
    assert!(
        typ.contains("#heading(level: 1"),
        "Typst 应含标题，实际：{typ}"
    );
}
