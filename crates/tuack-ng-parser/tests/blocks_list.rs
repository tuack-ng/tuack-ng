//! 列表测试：有序/无序列表（marker 变体、起始数字）、嵌套列表、
//! 缩进续行/空行分段/深层嵌套、任务列表语法禁用（`- [ ] a` 按普通文本解析）。

mod common;

use common::*;
use tuack_ng_parser::ast::BlockKind;
use tuack_ng_parser::ast::list::{ListBulletKind, ListKind};

#[test]
fn list_ordered() {
    let doc = tuack_ng_parser::parse("1. a");
    let kind = match &doc.blocks[0].value {
        BlockKind::List(list) => &list.kind,
        _ => panic!("应为列表"),
    };
    assert_eq!(kind, &ListKind::Ordered);
}

#[test]
fn list_ordered_start() {
    // `0100.` 是合法有序列表 marker；起始编号不记录（渲染恒为 1）。
    let doc = tuack_ng_parser::parse("0100. a");
    let kind = match &doc.blocks[0].value {
        BlockKind::List(list) => &list.kind,
        _ => panic!("应为列表"),
    };
    assert_eq!(kind, &ListKind::Ordered);
}

#[test]
fn list_ordered_paren() {
    let doc = tuack_ng_parser::parse("1) a");
    let kind = match &doc.blocks[0].value {
        BlockKind::List(list) => &list.kind,
        _ => panic!("应为列表"),
    };
    assert_eq!(kind, &ListKind::Ordered);
}

#[test]
fn list_bullet_dash() {
    assert_blocks(
        " -   a",
        vec![b(BlockKind::List(tuack_ng_parser::ast::List {
            kind: ListKind::Bullet(ListBulletKind::Dash),
            items: vec![li(vec![b(para(vec![text("a")]))])],
        }))],
    );
}

#[test]
fn list_bullet_star() {
    let doc = tuack_ng_parser::parse(" * list1\n * list1");
    match &doc.blocks[0].value {
        BlockKind::List(list) => {
            assert_eq!(list.kind, ListKind::Bullet(ListBulletKind::Star));
            assert_eq!(list.items.len(), 2);
        }
        _ => panic!("应为列表"),
    }
}

#[test]
fn list_bullet_plus() {
    let doc = tuack_ng_parser::parse("+ item");
    match &doc.blocks[0].value {
        BlockKind::List(list) => {
            assert_eq!(list.kind, ListKind::Bullet(ListBulletKind::Plus));
        }
        _ => panic!("应为列表"),
    }
}

#[test]
fn list_ordered_multi() {
    let doc = tuack_ng_parser::parse("1. a\n2. b");
    match &doc.blocks[0].value {
        BlockKind::List(list) => {
            assert_eq!(list.kind, ListKind::Ordered);
            assert_eq!(list.items.len(), 2);
        }
        _ => panic!("应为列表"),
    }
}

#[test]
fn task_list_syntax_not_parsed() {
    // 任务列表扩展已禁用：`- [ ] a` 的 `[ ]` 作为普通文本解析
    let doc = tuack_ng_parser::parse(" - [ ] a");
    match &doc.blocks[0].value {
        BlockKind::List(list) => {
            // 列表项是普通段落，无 task 字段
            assert!(list.items[0].value.blocks.len() == 1);
        }
        _ => panic!("应为列表"),
    }
}

#[test]
fn task_list_marker_is_text() {
    // `- [x] a` 中 `[x]` 是文本内容（勾选语义被禁用）
    let doc = tuack_ng_parser::parse(" - [x] a");
    let text: String = match &doc.blocks[0].value {
        BlockKind::List(list) => match &list.items[0].value.blocks[0].value {
            BlockKind::Paragraph(inlines) => inlines
                .iter()
                .map(|c| match &c.value {
                    tuack_ng_parser::ast::InlineKind::Text(t) => t.clone(),
                    _ => String::new(),
                })
                .collect(),
            _ => String::new(),
        },
        _ => panic!("应为列表"),
    };
    assert!(text.contains("[x]"), "`[x]` 应作为文本保留，实际：{text:?}");
}

#[test]
fn list_item_with_indented_continuation() {
    // 列表项内换行缩进：`  b` 属同一段落（软换行）。
    let doc = tuack_ng_parser::parse("- a\n  b\n");
    let list = match &doc.blocks[0].value {
        BlockKind::List(list) => list,
        other => panic!("应为列表，实际 {other:?}"),
    };
    assert_eq!(list.items.len(), 1, "应 1 个列表项");
    let blocks = &list.items[0].value.blocks;
    assert_eq!(blocks.len(), 1, "续行应属同一段落，实际 {blocks:#?}");
    let para = match &blocks[0].value {
        BlockKind::Paragraph(inlines) => inlines,
        other => panic!("应为段落，实际 {other:?}"),
    };
    assert!(
        para.iter()
            .any(|i| matches!(i.value, tuack_ng_parser::ast::InlineKind::SoftBreak)),
        "应含软换行（续行属同一段落），实际 {para:#?}"
    );
}

#[test]
fn list_item_blank_line_separated_paragraphs() {
    // 空行分隔：列表项内产生多个段落。
    let doc = tuack_ng_parser::parse("- a\n\n  b\n");
    let list = match &doc.blocks[0].value {
        BlockKind::List(list) => list,
        other => panic!("应为列表，实际 {other:?}"),
    };
    let blocks = &list.items[0].value.blocks;
    assert_eq!(blocks.len(), 2, "空行分隔应产生 2 个段落，实际 {blocks:#?}");
    assert!(
        matches!(&blocks[0].value, BlockKind::Paragraph(_)),
        "块 0 应为段落"
    );
    assert!(
        matches!(&blocks[1].value, BlockKind::Paragraph(_)),
        "块 1 应为段落"
    );
}

#[test]
fn list_nested_with_indent() {
    // 4 空格缩进 → 嵌套列表（列表项内段落 a + 嵌套 List b）。
    let doc = tuack_ng_parser::parse("- a\n    - b\n");
    let list = match &doc.blocks[0].value {
        BlockKind::List(list) => list,
        other => panic!("应为列表，实际 {other:?}"),
    };
    let blocks = &list.items[0].value.blocks;
    assert_eq!(
        blocks.len(),
        2,
        "列表项应含段落 a + 嵌套列表，实际 {blocks:#?}"
    );
    assert!(
        matches!(&blocks[0].value, BlockKind::Paragraph(_)),
        "块 0 应为段落 a"
    );
    let nested = match &blocks[1].value {
        BlockKind::List(list) => list,
        other => panic!("块 1 应为嵌套列表，实际 {other:?}"),
    };
    assert_eq!(nested.items.len(), 1, "嵌套列表应 1 项");
}

#[test]
fn list_deep_nesting() {
    // 深层缩进：多层嵌套列表。
    let doc = tuack_ng_parser::parse("- a\n    - b\n        - c\n");
    let l1 = match &doc.blocks[0].value {
        BlockKind::List(list) => list,
        other => panic!("应为列表，实际 {other:?}"),
    };
    let l2 = match &l1.items[0].value.blocks[1].value {
        BlockKind::List(list) => list,
        other => panic!("第 2 层应为列表，实际 {other:?}"),
    };
    let l3 = match &l2.items[0].value.blocks[1].value {
        BlockKind::List(list) => list,
        other => panic!("第 3 层应为列表，实际 {other:?}"),
    };
    assert_eq!(l3.items.len(), 1, "第 3 层列表应 1 项");
}

#[test]
fn list_excess_indent_is_not_nested() {
    // 缩进过深（6 空格）不构成嵌套列表：`- b` 成为段落内文本。
    let doc = tuack_ng_parser::parse("- a\n      - b\n");
    let list = match &doc.blocks[0].value {
        BlockKind::List(list) => list,
        other => panic!("应为列表，实际 {other:?}"),
    };
    let blocks = &list.items[0].value.blocks;
    assert_eq!(
        blocks.len(),
        1,
        "6 空格缩进不应产生嵌套列表，实际 {blocks:#?}"
    );
}
