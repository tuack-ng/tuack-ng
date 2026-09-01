//! 代码块测试：缩进代码块、围栏代码块（info 字符串、闭合 fence 长度、缩进保留）。

mod common;

use common::*;
use tuack_ng_parser::ast::block::CodeBlockKind;

#[test]
fn code_block_indented() {
    // 4 空格缩进代码块，内容保留 1 空格缩进；rushdown literal 带末尾换行
    assert_blocks("     a", vec![b(common::indented_code(" a\n"))]);
}

#[test]
fn code_block_fenced_no_info() {
    assert_blocks("```\na\n```", vec![b(common::code_block(None, "a\n"))]);
}

#[test]
fn code_block_fenced_longer() {
    // 闭合 fence 可更长
    assert_blocks(
        "`````\na\n`````````",
        vec![b(common::code_block(None, "a\n"))],
    );
}

#[test]
fn code_block_fenced_with_info() {
    assert_blocks(
        "```rust\na\n```",
        vec![b(common::code_block(Some("rust"), "a\n"))],
    );
}

#[test]
fn code_block_fenced_preserves_indent() {
    // fence 内相对缩进保留（literal 完整含前导空格）。
    let src = "```\n    a\n```\n";
    let doc = tuack_ng_parser::parse(src);
    let literal = match &doc.blocks[0].value {
        tuack_ng_parser::ast::BlockKind::CodeBlock(cb) => &cb.literal,
        _ => panic!("应解析为代码块"),
    };
    assert_eq!(literal, "    a\n", "literal 应完整保留前导缩进");
}

#[test]
fn code_block_info_variant() {
    let doc = tuack_ng_parser::parse("```c++\nint x;\n```\n");
    match &doc.blocks[0].value {
        tuack_ng_parser::ast::BlockKind::CodeBlock(cb) => match &cb.kind {
            CodeBlockKind::Fenced { info } => {
                assert_eq!(info.as_deref(), Some("c++"));
            }
            _ => panic!("应为 fenced"),
        },
        _ => panic!("应解析为代码块"),
    }
}
