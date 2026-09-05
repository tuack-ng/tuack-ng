//! serde 往返测试：AST 经 JSON 序列化再反序列化后与原 AST 完全一致。
//!
//! 仅当开启 `serde` feature 时编译运行（`cargo test -p tuack-ng-parser --features serde`）。

#![cfg(feature = "serde")]

use tuack_ng_parser::ast::Document;

fn roundtrip(source: &str) {
    let doc = tuack_ng_parser::parse(source);
    let json = serde_json::to_string(&doc).expect("序列化失败");
    let back: Document = serde_json::from_str(&json).expect("反序列化失败");
    assert_eq!(doc, back, "往返不一致，source: {source:?}");
}

#[test]
fn serde_json_roundtrip_comprehensive() {
    roundtrip(include_str!("fixtures/comprehensive.md"));
}

/// 覆盖 comprehensive.md 未涉及的枚举变体：Setext 标题、引用式链接、
/// 脚注、缩进代码块、HTML 块、链接定义、硬换行等。
#[test]
fn serde_json_roundtrip_extra_variants() {
    roundtrip(
        "Title\n===\n\n[text][ref] and [text][] and [text]\n\n[ref]: https://example.com \"title\"\n\n    indented code\n\n<div>html</div>\n\nfootnote[^1]\n\n[^1]: footnote body\n\nhard  \nbreak\n",
    );
}
