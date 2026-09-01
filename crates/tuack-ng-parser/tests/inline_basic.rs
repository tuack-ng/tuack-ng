//! 行内元素测试：强调/加粗/删除线（含嵌套与下划线边界）、链接（内联/尖括号 URL/嵌套图片）、
//! 图片（属性/title）、Autolink、行内代码、软/硬换行节点。

mod common;

use common::*;
use tuack_ng_parser::ast::{BlockKind, InlineKind};

#[test]
fn emphasis_basic() {
    assert_blocks(
        "*foo bar*",
        vec![b(para(vec![emphasis(vec![text("foo bar")])]))],
    );
    assert_blocks("_foo_", vec![b(para(vec![emphasis(vec![text("foo")])]))]);
}

#[test]
fn emphasis_nested_underscore() {
    // rushdown 把 ___bar___ 解析为 Emphasis[Strong[bar]]
    assert_blocks(
        "foo ___bar___",
        vec![b(para(vec![
            text("foo "),
            emphasis(vec![strong(vec![text("bar")])]),
        ]))],
    );
}

#[test]
fn strong_followed_by_text() {
    assert_blocks(
        "**foo**bar",
        vec![b(para(vec![strong(vec![text("foo")]), text("bar")]))],
    );
}

#[test]
fn emphasis_underscore_in_word() {
    // PKG_CONFIG_PATH 不应被解析为强调；rushdown 会按下划线切分 Text
    assert_blocks(
        "Note that we set PKG_CONFIG_PATH only if it's not _already_ set",
        vec![b(para(vec![
            text("Note that we set PKG_"),
            text("CONFIG_"),
            text("PATH only if it's not "),
            emphasis(vec![text("already")]),
            text(" set"),
        ]))],
    );
}

#[test]
fn strikethrough_basic() {
    assert_blocks(
        "~~deleted~~",
        vec![b(para(vec![strikethrough(vec![text("deleted")])]))],
    );
}

#[test]
fn inline_link() {
    assert_blocks(
        r#"[foo](/url "title")"#,
        vec![b(para(vec![link(
            "/url",
            Some("title"),
            vec![text("foo")],
        )]))],
    );
}

#[test]
fn inline_link_angle_bracket_url() {
    assert_blocks(
        "[foo](<url>)",
        vec![b(para(vec![link("url", None, vec![text("foo")])]))],
    );
}

#[test]
fn inline_link_nested_image() {
    // GitHub badge pattern
    assert_blocks(
        "[![userstyles](https://img.shields.io/badge/userstyles-green)](https://userstyles.world/user/Paul-16098)",
        vec![b(para(vec![link(
            "https://userstyles.world/user/Paul-16098",
            None,
            vec![image(
                "https://img.shields.io/badge/userstyles-green",
                None,
                "userstyles",
                None,
            )],
        )]))],
    );
}

#[test]
fn inline_link_multiple_images() {
    assert_blocks(
        "[![a](url1) ![b](url2)](main-url)",
        vec![b(para(vec![link(
            "main-url",
            None,
            vec![
                image("url1", None, "a", None),
                text(" "),
                image("url2", None, "b", None),
            ],
        )]))],
    );
}

#[test]
fn image_basic() {
    assert_blocks(
        r#"![foo](/url "title")"#,
        vec![b(para(vec![image("/url", Some("title"), "foo", None)]))],
    );
    assert_blocks(
        "![foo](train.jpg)",
        vec![b(para(vec![image("train.jpg", None, "foo", None)]))],
    );
}

#[test]
fn image_empty_alt() {
    assert_blocks(
        "![](train.jpg)",
        vec![b(para(vec![image("train.jpg", None, "", None)]))],
    );
}

#[test]
fn image_with_attributes() {
    assert_blocks(
        r#"![foo](/url){width="100pt" height="50pt"}"#,
        vec![b(para(vec![image(
            "/url",
            None,
            "foo",
            Some(attr(Some("100pt"), Some("50pt"))),
        )]))],
    );
}

#[test]
fn image_with_attributes_and_title() {
    assert_blocks(
        r#"![foo](/url "title"){width="100pt" height="50pt"}"#,
        vec![b(para(vec![image(
            "/url",
            Some("title"),
            "foo",
            Some(attr(Some("100pt"), Some("50pt"))),
        )]))],
    );
}

#[test]
fn autolink() {
    // <https://...> 分转为 Autolink variant
    assert_blocks(
        "<https://example.com>",
        vec![b(para(vec![i(InlineKind::Autolink(
            tuack_ng_parser::ast::Autolink {
                url: "https://example.com".to_string(),
                text: "<https://example.com>".to_string(),
            },
        ))]))],
    );
}

#[test]
fn inline_code() {
    assert_blocks("`code here`", vec![b(para(vec![code("code here")]))]);
}

#[test]
fn hard_line_break() {
    // 行尾两个空格 → 硬换行
    let doc = tuack_ng_parser::parse("a  \nb");
    match &doc.blocks[0].value {
        BlockKind::Paragraph(inlines) => {
            assert!(inlines.len() >= 2, "应有换行拆分");
        }
        _ => panic!("应为段落"),
    }
}

#[test]
fn hard_line_break_node() {
    // 反斜杠行尾也产生硬换行；渲染时应保留硬换行标记
    let doc = tuack_ng_parser::parse("a\\\nb");
    let has_linebreak = match &doc.blocks[0].value {
        BlockKind::Paragraph(inlines) => inlines
            .iter()
            .any(|i| matches!(i.value, InlineKind::LineBreak)),
        _ => false,
    };
    assert!(has_linebreak, "反斜杠硬换行应产生 LineBreak 节点");
    let out = tuack_ng_parser::printers::render_markdown(&doc);
    assert!(out.contains("  \n"), "渲染应保留硬换行标记，实际：{out:?}");
}

#[test]
fn soft_break_node() {
    // 普通换行 → SoftBreak 节点
    let doc = tuack_ng_parser::parse("a\nb");
    let has_softbreak = match &doc.blocks[0].value {
        BlockKind::Paragraph(inlines) => inlines
            .iter()
            .any(|i| matches!(i.value, InlineKind::SoftBreak)),
        _ => false,
    };
    assert!(has_softbreak, "普通换行应产生 SoftBreak 节点");
}
