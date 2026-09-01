//! Markdown 渲染往返测试（移植自 markdown-ppp printer/tests）。
//!
//! 验证 parse -> render_markdown 的一致性：解析后的 AST 渲染回 Markdown，
//! 再解析一次应得到相同结构（幂等性）。

mod common;

use tuack_ng_parser::printers::render_markdown;

/// 解析 -> 渲染 -> 再解析，两次 AST 结构应一致（忽略 span）。
fn round_trip_is_idempotent(source: &str) {
    let doc1 = tuack_ng_parser::parse(source);
    let md = render_markdown(&doc1);
    let doc2 = tuack_ng_parser::parse(&md);

    let mut b1 = doc1.blocks;
    let mut b2 = doc2.blocks;
    common::strip_public_spans(&mut b1);
    common::strip_public_spans(&mut b2);
    assert_eq!(b1, b2, "往返不幂等。source: {source:?}\nmd: {md:?}");
}

#[test]
fn roundtrip_paragraphs() {
    round_trip_is_idempotent("word1 word2");
    round_trip_is_idempotent("paragraph1\n\nparagraph2");
}

#[test]
fn roundtrip_headings() {
    round_trip_is_idempotent("# heading1\n\n## heading2");
}

#[test]
fn roundtrip_emphasis() {
    round_trip_is_idempotent("Это *курсив, но внутри **жирный*** снова.");
    round_trip_is_idempotent("Это \\*не курсив\\*, а просто звёздочки.");
}

#[test]
fn roundtrip_link() {
    round_trip_is_idempotent("[ссылка с *курсивом внутри*](https://example.com)");
}

#[test]
fn roundtrip_code() {
    round_trip_is_idempotent("Инлайн код `внутри *курсива*`.");
}

#[test]
fn roundtrip_table() {
    round_trip_is_idempotent(
        "| Заголовок 1 | Заголовок 2 | Заголовок 3 |\n| ----------- | ----------: | :---------: |\n| Ячейка 1    |    Ячейка 2 |  Ячейка 3   |",
    );
}

#[test]
fn roundtrip_nested_list() {
    round_trip_is_idempotent(
        " 1. item 1\n\n     * nested list item 1\n     * nested list item 2\n 2. item 2",
    );
}

#[test]
fn roundtrip_blockquote() {
    round_trip_is_idempotent("> line1 line1 line1");
    // 多行 blockquote：软换行保留，应幂等往返
    round_trip_is_idempotent("> line1 line1 line1\n> line1 line1 line1");
    round_trip_is_idempotent("> line1 line1\n> > line2 line2");
}

#[test]
fn roundtrip_fenced_code() {
    round_trip_is_idempotent("text\n\n```rust\nlet s = \"hello\";\n```");
}

#[test]
fn roundtrip_table_alignment() {
    round_trip_is_idempotent(
        "| Header1 | Header2 | Header3 |\n| ------- | ------- | ------- |\n| Cell1   | Cell2   | Cell3   |",
    );
}

#[test]
fn roundtrip_merged_table() {
    // 合并表格往返：MD 渲染不处理合并（`<`/`^` 保留），应能幂等读回
    round_trip_is_idempotent("| A1 | < | A3 |\n| --- | --- | --- |\n| B1 | B2 | ^ |");
}

#[test]
fn roundtrip_reference_links() {
    // 三种引用式链接形式 + 定义，应保真往返
    round_trip_is_idempotent("[foo][ref]\n\n[ref]: https://example.com");
    round_trip_is_idempotent("[foo][]\n\n[foo]: https://example.com");
    round_trip_is_idempotent("[ref]\n\n[ref]: https://example.com");
}

#[test]
fn roundtrip_autolink() {
    round_trip_is_idempotent("Visit <https://example.com> for details");
}

#[test]
fn roundtrip_align_container() {
    // 对齐容器（裸参数）应保真往返：`:::align{right}` 渲染回不带 `=""`。
    round_trip_is_idempotent(":::align{right}\ntext\n:::");
    round_trip_is_idempotent(":::align{center}\ntext\n:::");
    round_trip_is_idempotent(":::figure{caption=cap}\ntext\n:::");
}

#[test]
fn roundtrip_container_param_escaping() {
    // KeyValue 值含 `"`/`&`/换行时，打印端应重编码为实体以保证往返幂等。
    round_trip_is_idempotent(":::figure{caption=\"A &quot;B&quot; C\"}\ntext\n:::");
    round_trip_is_idempotent(":::figure{caption=\"x &amp; y\"}\ntext\n:::");
    round_trip_is_idempotent(":::figure{caption=\"a&#10;b\"}\ntext\n:::");
}

#[test]
fn roundtrip_container_kind_escaping() {
    // kind 含空格（多 class）、为空、含实体解码字符时，打印端应改用 class 属性形式，
    // 保证再解析不炸。
    round_trip_is_idempotent(":::{.a .b}\ntext\n:::");
    round_trip_is_idempotent(":::{.}\ntext\n:::");
    round_trip_is_idempotent(":::figure{class=\"a&amp;b\"}\ntext\n:::");
    round_trip_is_idempotent(":::{.a .b, key=v}\ntext\n:::");
}
