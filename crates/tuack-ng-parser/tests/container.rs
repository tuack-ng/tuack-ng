//! fenced-div 容器测试：基础、嵌套、带参数。

mod common;

use common::*;
use tuack_ng_parser::ast::BlockKind;

#[test]
fn container_basic() {
    assert_blocks(
        ":::note\n内容\n:::",
        vec![b(BlockKind::Container(tuack_ng_parser::ast::Container {
            kind: "note".to_string(),
            params: vec![],
            blocks: vec![b(para(vec![text("内容")]))],
        }))],
    );
}

#[test]
fn container_nested() {
    // 嵌套容器：外层 a 内层 b
    let doc = tuack_ng_parser::parse(":::a\n:::b\ninner\n:::\n:::\n");
    let outer = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("外层应为 Container，实际 {other:?}"),
    };
    assert_eq!(outer.kind, "a");
    assert_eq!(outer.blocks.len(), 1, "外层应含 1 个内层容器");
    let inner = match &outer.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("内层应为 Container，实际 {other:?}"),
    };
    assert_eq!(inner.kind, "b");
    assert_eq!(inner.blocks.len(), 1, "内层应含 1 个段落");
}

#[test]
fn container_nested_with_params() {
    // 外层带参数 + 内层嵌套
    let doc = tuack_ng_parser::parse(":::a{key=val}\n:::b\ninner\n:::\n:::\n");
    let outer = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("应为 Container，实际 {other:?}"),
    };
    assert_eq!(outer.kind, "a");
    assert!(
        outer.params.iter().any(|(k, v)| k == "key" && v == "val"),
        "外层应含 key=val，实际 {:?}",
        outer.params
    );
    assert!(matches!(&outer.blocks[0].value, BlockKind::Container(c) if c.kind == "b"));
}

#[test]
fn container_mixed_content() {
    // 容器内多段落 + 嵌套容器 + 尾部段落
    let doc = tuack_ng_parser::parse(":::a\npara1\n\n:::b\npara2\n:::\n\npara3\n:::\n");
    let outer = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("应为 Container，实际 {other:?}"),
    };
    assert_eq!(
        outer.blocks.len(),
        3,
        "外层应含 3 块，实际 {:?}",
        outer.blocks
    );
    assert!(
        matches!(&outer.blocks[0].value, BlockKind::Paragraph(_)),
        "块 0 应为段落"
    );
    assert!(
        matches!(&outer.blocks[1].value, BlockKind::Container(_)),
        "块 1 应为容器"
    );
    assert!(
        matches!(&outer.blocks[2].value, BlockKind::Paragraph(_)),
        "块 2 应为段落"
    );
}

#[test]
fn container_pandoc_style() {
    // Pandoc 风格 `::: {.kind key=val}`
    let doc = tuack_ng_parser::parse("::: {.figure caption=\"t\"}\n内容\n:::\n");
    let c = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("应为 Container，实际 {other:?}"),
    };
    assert_eq!(c.kind, "figure");
    assert!(c.params.iter().any(|(k, v)| k == "caption" && v == "t"));
}
