//! 引用块（Blockquote）测试：基础、嵌套、深层嵌套。

mod common;

use common::*;

#[test]
fn blockquote_basic() {
    assert_blocks("> a", vec![b(blockquote(vec![b(para(vec![text("a")]))]))]);
}

#[test]
fn blockquote_nested() {
    // 外层引用包含段落 a 和嵌套引用 b
    assert_blocks(
        "> a\n>\n>> b",
        vec![b(blockquote(vec![
            b(para(vec![text("a")])),
            b(blockquote(vec![b(para(vec![text("b")]))])),
        ]))],
    );
}

#[test]
fn blockquote_deep() {
    assert_blocks(
        ">> a\n>>\n>> b",
        vec![b(blockquote(vec![b(blockquote(vec![
            b(para(vec![text("a")])),
            b(para(vec![text("b")])),
        ]))]))],
    );
}
