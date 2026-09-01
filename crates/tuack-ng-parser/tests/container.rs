//! fenced-div 容器测试：基础、嵌套、带参数。

mod common;

use common::*;
use tuack_ng_parser::ast::BlockKind;
use tuack_ng_parser::ast::block::ContainerParam;

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
        outer
            .params
            .iter()
            .any(|p| p.key() == "key" && p.value() == Some("val")),
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
    assert!(
        c.params
            .iter()
            .any(|p| p.key() == "caption" && p.value() == Some("t")),
        "应含 caption=t，实际 {:?}",
        c.params
    );
}

#[test]
fn container_bare_param() {
    // 无值裸参数 `:::align{right}` -> Flag("right")。
    for src in [
        ":::align{right}\n内容\n:::\n",
        ":::align {right}\n内容\n:::\n",
    ] {
        let doc = tuack_ng_parser::parse(src);
        let c = match &doc.blocks[0].value {
            BlockKind::Container(c) => c,
            other => panic!("应为 Container，实际 {other:?}"),
        };
        assert_eq!(c.kind, "align");
        assert!(
            c.params
                .iter()
                .any(|p| p.key() == "right" && p.value().is_none()),
            "应含裸参数 right，实际 {:?}",
            c.params
        );
    }
}

#[test]
fn container_mixed_bare_and_keyvalue() {
    // 混合列表 `{aa, bb, b=c, c=d}`：裸属性与键值对共存。
    let doc = tuack_ng_parser::parse(":::a{aa, bb, b=c, c=d}\n内容\n:::\n");
    let c = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("应为 Container，实际 {other:?}"),
    };
    assert_eq!(c.kind, "a");
    assert_eq!(
        c.params,
        vec![
            ContainerParam::Flag("aa".to_string()),
            ContainerParam::Flag("bb".to_string()),
            ContainerParam::KeyValue("b".to_string(), "c".to_string()),
            ContainerParam::KeyValue("c".to_string(), "d".to_string()),
        ]
    );
}

#[test]
fn container_empty_quoted_value() {
    // `key=""` 是显式空值，应保留为 KeyValue("","")，而非误判为裸 Flag。
    let doc = tuack_ng_parser::parse(":::figure{caption=\"\"}\n内容\n:::\n");
    let c = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("应为 Container，实际 {other:?}"),
    };
    assert_eq!(c.kind, "figure");
    assert_eq!(
        c.params,
        vec![ContainerParam::KeyValue(
            "caption".to_string(),
            String::new()
        )]
    );
}

#[test]
fn container_entity_reference() {
    // 属性值中的 HTML 实体/数字引用应被解析（与 rushdown parse_attributes 一致）。
    let doc = tuack_ng_parser::parse(":::figure{caption=\"A &amp; B &quot;C&quot;\"}\n内容\n:::\n");
    let c = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!("应为 Container，实际 {other:?}"),
    };
    assert_eq!(
        c.params,
        vec![ContainerParam::KeyValue(
            "caption".to_string(),
            "A & B \"C\"".to_string()
        )]
    );
}

#[test]
fn container_empty_id_and_class() {
    // 与 rushdown parse_attributes 一致：`{#}` / `{.}` 接受空 id/class，不应退化。
    let doc = tuack_ng_parser::parse(":::{#}\n内容\n:::\n");
    let c = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!(":::# 应为 Container，实际 {other:?}"),
    };
    assert_eq!(c.kind, "");
    assert_eq!(
        c.params,
        vec![ContainerParam::KeyValue("id".to_string(), String::new())]
    );

    let doc = tuack_ng_parser::parse(":::{.}\n内容\n:::\n");
    let c = match &doc.blocks[0].value {
        BlockKind::Container(c) => c,
        other => panic!(":::. 应为 Container，实际 {other:?}"),
    };
    assert_eq!(c.kind, "");
    assert!(c.params.is_empty(), ":::. 应无参数，实际 {:?}", c.params);
}
