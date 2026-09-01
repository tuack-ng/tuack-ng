//! 异常输入（sad-path）测试：空文档、未闭合语法、深层嵌套等，应不 panic 且合理降级。

use tuack_ng_parser::ast::BlockKind;

#[test]
fn sad_empty_document() {
    let doc = tuack_ng_parser::parse("");
    assert!(doc.blocks.is_empty());
}

#[test]
fn sad_blank_lines_only() {
    let doc = tuack_ng_parser::parse("\n\n\n");
    assert!(doc.blocks.is_empty());
}

#[test]
fn sad_unclosed_emphasis() {
    // 未闭合强调按文本处理，不应 panic
    let doc = tuack_ng_parser::parse("*foo");
    assert!(!doc.blocks.is_empty());
}

#[test]
fn sad_unclosed_code_fence() {
    // 未闭合围栏代码块
    let doc = tuack_ng_parser::parse("```\nfoo");
    assert!(!doc.blocks.is_empty());
}

#[test]
fn sad_unclosed_link() {
    // 未闭合链接按文本处理
    let doc = tuack_ng_parser::parse("[foo](url");
    assert!(!doc.blocks.is_empty());
}

#[test]
fn sad_deep_nesting() {
    // 深层嵌套引用
    let deep = "> ".repeat(20) + "text";
    let doc = tuack_ng_parser::parse(&deep);
    assert!(!doc.blocks.is_empty());
}

#[test]
fn sad_table_header_wider_than_body() {
    let doc = tuack_ng_parser::parse("| a | b | c |\n| - | - | - |\n| x |\n");
    let table = doc.blocks.iter().find_map(|b| match &b.value {
        BlockKind::Table(t) => Some(t),
        _ => None,
    });
    assert!(table.is_some(), "应解析为表格");
}

#[test]
fn sad_empty_cell() {
    let doc = tuack_ng_parser::parse("| a |  | c |\n| - | - | - |\n| x | y | z |\n");
    assert!(matches!(&doc.blocks[0].value, BlockKind::Table(_)));
}

#[test]
fn sad_html_entity() {
    // HTML 实体应解析为文本
    let doc = tuack_ng_parser::parse("a &amp; b");
    assert!(!doc.blocks.is_empty());
}

#[test]
fn sad_unicode_cjk() {
    let doc = tuack_ng_parser::parse("# 中文标题\n\n这是**加粗**的中文。");
    assert!(!doc.blocks.is_empty());
}

#[test]
fn sad_empty_container() {
    let doc = tuack_ng_parser::parse(":::a\n:::\n");
    assert!(matches!(&doc.blocks[0].value, BlockKind::Container(_)));
}
