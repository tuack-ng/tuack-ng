//! HTML 测试：块级 HTML（HtmlBlock）与行内 HTML（RawHtml）。

use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;

#[test]
fn html_block() {
    let doc = tuack_ng_parser::parse("<div>\n<p>hi</p>\n</div>");
    assert!(matches!(&doc.blocks[0].value, BlockKind::HtmlBlock(_)));
}

#[test]
fn html_inline() {
    let doc = tuack_ng_parser::parse("text <span>x</span>");
    match &doc.blocks[0].value {
        BlockKind::Paragraph(inlines) => {
            assert!(
                inlines
                    .iter()
                    .any(|i| matches!(&i.value, InlineKind::Html(_)))
            );
        }
        _ => panic!("应为段落"),
    }
}
