//! 标题测试：ATX 标题（级别、空格要求）、Setext 标题、标题内行内内容，
//! 以及 Setext / ThematicBreak 歧义解析。

mod common;

use common::*;
use tuack_ng_parser::ast::BlockKind;

#[test]
fn heading_v1() {
    assert_blocks("# 标题", vec![b(heading_atx(1, vec![text("标题")]))]);
    // ATX 与标题之间必须有空格（默认行为）
    assert_blocks("##a", vec![b(para(vec![text("##a")]))]);
}

#[test]
fn heading_levels() {
    assert_blocks("###### 六级", vec![b(heading_atx(6, vec![text("六级")]))]);
    // 超过 6 个 # 不是标题；rushdown 会切成两个文本节点
    assert_blocks(
        "####### 七个",
        vec![b(para(vec![text("#######"), text(" 七个")]))],
    );
}

#[test]
fn heading_setext_test() {
    assert_blocks("a\n==", vec![b(setext_heading(1, vec![text("a")]))]);
    assert_blocks("a\n--", vec![b(setext_heading(2, vec![text("a")]))]);
}

#[test]
fn heading_strong_content() {
    assert_blocks(
        "## **加粗**",
        vec![b(heading_atx(2, vec![strong(vec![text("加粗")])]))],
    );
}

// ---- Setext / ThematicBreak 歧义（CommonMark） ----

#[test]
fn setext_vs_thematic_break_after_paragraph() {
    // 段落紧跟 `=` → Setext H1
    assert_blocks("a\n===", vec![b(setext_heading(1, vec![text("a")]))]);
    // 段落紧跟 `-` → Setext H2
    assert_blocks("a\n---", vec![b(setext_heading(2, vec![text("a")]))]);
    // 独立 `---` → ThematicBreak
    assert_blocks("---", vec![b(BlockKind::ThematicBreak)]);
    // `***` 不是 Setext 下划线（只认 =/-），独立时是分隔线
    assert_blocks(
        "a\n***",
        vec![b(para(vec![text("a")])), b(BlockKind::ThematicBreak)],
    );
    // `- -` 不是分隔线也不是 Setext，是列表（第二个 `-` 为内层列表）。
    let doc = tuack_ng_parser::parse("a\n- -");
    assert!(
        !doc.blocks
            .iter()
            .any(|b| matches!(b.value, BlockKind::ThematicBreak | BlockKind::Heading(_))),
        "`- -` 不应是分隔线或标题，实际 {:#?}",
        doc.blocks
    );
    // 第二个块是 List；其首个列表项内应嵌套一个 List（第二个 `-`）。
    let outer = &doc.blocks[1];
    let nested_is_list = match &outer.value {
        BlockKind::List(l) => l
            .items
            .first()
            .is_some_and(|item| matches!(&item.value.blocks[0].value, BlockKind::List(_))),
        _ => false,
    };
    assert!(
        nested_is_list,
        "`- -` 应解析为外层 List 内嵌 List，实际 {:#?}",
        outer
    );
}
