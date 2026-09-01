//! 基础块测试：段落、多段落、软换行、分隔线（ThematicBreak）。

mod common;

use common::*;
use tuack_ng_parser::ast::BlockKind;

#[test]
fn minimal_paragraph() {
    assert_blocks("a", vec![b(para(vec![text("a")]))]);
    // rushdown 把空格分隔的 token 合并进前一个 Text 节点
    assert_blocks("a b c", vec![b(para(vec![text("a b"), text(" c")]))]);
    // 换行拆分 Text 节点；软换行为 SoftBreak 节点
    assert_blocks(
        "a\nb\nc",
        vec![b(para(vec![
            text("a"),
            soft_break(),
            text("b"),
            soft_break(),
            text("c"),
        ]))],
    );
}

#[test]
fn multi_paragraph() {
    assert_blocks(
        "a\n\nb",
        vec![b(para(vec![text("a")])), b(para(vec![text("b")]))],
    );
    assert_blocks(
        "a\n\n\n\n\nb",
        vec![b(para(vec![text("a")])), b(para(vec![text("b")]))],
    );
    assert_blocks(
        "a\n\n  b",
        vec![b(para(vec![text("a")])), b(para(vec![text("b")]))],
    );
}

#[test]
fn thematic_break_basic() {
    assert_blocks("---", vec![b(BlockKind::ThematicBreak)]);
    assert_blocks("***", vec![b(BlockKind::ThematicBreak)]);
    assert_blocks("___", vec![b(BlockKind::ThematicBreak)]);
    // 带空格的
    assert_blocks("--- ---", vec![b(BlockKind::ThematicBreak)]);
}

#[test]
fn parse_basic() {
    // 混合结构冒烟：标题 + 段落（含强调/加粗）+ 列表。
    let doc = tuack_ng_parser::parse("# 标题\n\n段落 *强调* 和 **加粗**。\n\n- 列表项\n- 第二项\n");
    assert!(
        doc.blocks.len() >= 3,
        "期望 3+ 块，实际 {}",
        doc.blocks.len()
    );
}
