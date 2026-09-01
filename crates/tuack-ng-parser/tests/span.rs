//! 全元素 span 覆盖测试。
//!
//! 约定：
//! - 块元素与复合行内元素：span 可为 `None`，若为 `Some` 则仅要求 `start` 精确。
//! - 叶子行内元素（Text/Code/Latex/Html）：span 必须为 `Some`，且 `start` 指向内容起始。

use tuack_ng_parser::Span;
use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::visitor::{VisitWith, Visitor};

/// 收集的节点：kind 名称 + 行内/块 + span + 内容文本。
#[derive(Debug)]
struct Item {
    /// 元素 kind 名称（如 "ThematicBreak"、"Text"、"Code"）。
    name: &'static str,
    span: Option<Span>,
    /// 元素携带的内容（Text/Code/Latex/Html 的字符串内容）。
    content: Option<String>,
}

/// 在 `src` 中查找 `needle` 首次出现的字节偏移。
fn offset_of(src: &str, needle: &str) -> usize {
    src.find(needle)
        .unwrap_or_else(|| panic!("未找到 {needle:?}"))
}

/// 断言节点 span 的 start 指向期望内容。
fn assert_start(span: Option<Span>, src: &str, expect: &str, what: &str) {
    let span = span.unwrap_or_else(|| panic!("{what} 应有 span"));
    let expect_off = offset_of(src, expect);
    assert_eq!(
        span.start, expect_off,
        "{what}: start 应为 {expect_off}（{expect:?}），实际 {}",
        span.start
    );
}

/// 块 kind → 名称。
fn block_name(k: &BlockKind) -> &'static str {
    match k {
        BlockKind::Paragraph(_) => "Paragraph",
        BlockKind::Heading(_) => "Heading",
        BlockKind::ThematicBreak => "ThematicBreak",
        BlockKind::BlockQuote(_) => "BlockQuote",
        BlockKind::List(_) => "List",
        BlockKind::CodeBlock(_) => "CodeBlock",
        BlockKind::HtmlBlock(_) => "HtmlBlock",
        BlockKind::Definition(_) => "Definition",
        BlockKind::Table(_) => "Table",
        BlockKind::FootnoteDefinition(_) => "FootnoteDefinition",
        BlockKind::Container(_) => "Container",
        BlockKind::LatexBlock(_) => "LatexBlock",
        BlockKind::Empty => "Empty",
    }
}

/// 行内 kind → 名称。
fn inline_name(k: &InlineKind) -> &'static str {
    match k {
        InlineKind::Text(_) => "Text",
        InlineKind::SoftBreak => "SoftBreak",
        InlineKind::LineBreak => "LineBreak",
        InlineKind::Code(_) => "Code",
        InlineKind::Latex(_) => "Latex",
        InlineKind::Html(_) => "Html",
        InlineKind::Link(_) => "Link",
        InlineKind::LinkReference(_) => "LinkReference",
        InlineKind::Image(_) => "Image",
        InlineKind::Emphasis(_) => "Emphasis",
        InlineKind::Strong(_) => "Strong",
        InlineKind::Strikethrough(_) => "Strikethrough",
        InlineKind::Autolink(_) => "Autolink",
        InlineKind::FootnoteReference(_) => "FootnoteReference",
        InlineKind::Empty => "Empty",
    }
}

fn collect(src: &str) -> Vec<Item> {
    struct C {
        out: Vec<Item>,
    }
    impl Visitor for C {
        fn visit_block(&mut self, b: &tuack_ng_parser::Block) {
            let content = match &b.value {
                BlockKind::CodeBlock(cb) => Some(cb.literal.clone()),
                BlockKind::HtmlBlock(h) => Some(h.clone()),
                BlockKind::LatexBlock(l) => Some(l.clone()),
                _ => None,
            };
            self.out.push(Item {
                name: block_name(&b.value),
                span: b.span,
                content,
            });
            self.walk_block(b);
        }
        fn visit_inline(&mut self, i: &tuack_ng_parser::Inline) {
            let content = match &i.value {
                InlineKind::Text(t) => Some(t.clone()),
                InlineKind::Code(c) => Some(c.clone()),
                InlineKind::Latex(l) => Some(l.clone()),
                InlineKind::Html(h) => Some(h.clone()),
                InlineKind::Autolink(a) => Some(format!("<{}>", a.text)),
                InlineKind::Image(img) => Some(format!("![{}]", img.alt)),
                _ => None,
            };
            self.out.push(Item {
                name: inline_name(&i.value),
                span: i.span,
                content,
            });
            self.walk_inline(i);
        }
    }
    let doc = tuack_ng_parser::parse(src);
    let mut c = C { out: Vec::new() };
    doc.visit_with(&mut c);
    c.out
}

fn first<'a>(out: &'a [Item], name: &str) -> &'a Item {
    out.iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("未找到 {name}: {out:?}"))
}

// ---- 块元素 ----

#[test]
fn block_thematic_break_start() {
    let src = "abc\n\n---\n";
    let out = collect(src);
    assert_start(
        first(&out, "ThematicBreak").span,
        src,
        "---",
        "ThematicBreak",
    );
}

#[test]
fn block_code_fenced_start() {
    let src = "abc\n\n```rust\ncode\n```\n";
    let out = collect(src);
    // fenced：start 应指向 fence 开头的 ```（而非内容）
    assert_start(
        first(&out, "CodeBlock").span,
        src,
        "```rust",
        "CodeBlock(fenced)",
    );
}

#[test]
fn block_code_indented_start() {
    let src = "abc\n\n    indented\n";
    let out = collect(src);
    // indented：start 应指向内容（缩进后的首字符）
    assert_start(
        first(&out, "CodeBlock").span,
        src,
        "indented",
        "CodeBlock(indented)",
    );
}

#[test]
fn block_html_start() {
    let src = "abc\n\n<div>\nraw\n</div>\n";
    let out = collect(src);
    assert_start(first(&out, "HtmlBlock").span, src, "<div>", "HtmlBlock");
}

#[test]
fn block_latex_start() {
    let src = "abc\n\n$$\nformula\n$$\n";
    let out = collect(src);
    assert_start(first(&out, "LatexBlock").span, src, "$$", "LatexBlock");
}

#[test]
fn block_definition_start() {
    let src = "abc\n\n[ref]: https://x.io\n";
    let out = collect(src);
    assert_start(first(&out, "Definition").span, src, "[ref]", "Definition");
}

#[test]
fn definition_label_is_plain_text() {
    // CommonMark：定义 label 是纯文本标识符（含 `*` 等字符但不解析行内语法）。
    let src = "[*foo* bar]: /url \"title\"\n";
    let doc = tuack_ng_parser::parse(src);
    let def = match &doc.blocks[0].value {
        BlockKind::Definition(d) => d,
        other => panic!("应为 Definition，实际 {other:?}"),
    };
    assert_eq!(def.label, "*foo* bar", "label 应为含 `*` 的原始文本");
    assert_eq!(def.destination, "/url");
    assert_eq!(def.title.as_deref(), Some("title"));
    // 渲染应原样输出 label（不把 `*` 当强调）。
    let md = tuack_ng_parser::printers::render_markdown(&doc);
    assert!(
        md.contains("[*foo* bar]: /url"),
        "渲染应保留原始 label，实际 {md:?}"
    );
}

#[test]
fn block_all_containers_have_start() {
    let src = concat!(
        "para\n",
        "\n",
        "# heading\n",
        "\n",
        "- item\n",
        "\n",
        "| a |\n| - |\n| b |\n",
        "\n",
        "> quote\n",
        "\n",
        ":::info\n",
        "content\n",
        ":::\n",
    );
    let out = collect(src);
    // 每个容器块都应有精确 start（指向其内容起始）。
    assert_start(first(&out, "Paragraph").span, src, "para", "Paragraph");
    assert_start(first(&out, "Heading").span, src, "# heading", "Heading");
    assert_start(first(&out, "List").span, src, "- item", "List");
    assert_start(first(&out, "Table").span, src, "| a |", "Table");
    assert_start(first(&out, "BlockQuote").span, src, "> quote", "BlockQuote");
    assert_start(first(&out, "Container").span, src, ":::info", "Container");
}

// ---- 叶子行内元素 ----

#[test]
fn inline_text_start() {
    let src = "xx 你好 世界 yy\n";
    let out = collect(src);
    // 每个 Text 的 span 切片都应精确等于其内容。
    for item in out.iter().filter(|i| i.name == "Text") {
        let span = item.span.expect("Text 应有 span");
        let content = item.content.as_ref().expect("Text 有内容");
        assert_eq!(
            &src[span.start..span.stop],
            content,
            "Text span 应精确覆盖内容"
        );
    }
}

#[test]
fn inline_code_start() {
    let src = "a `code` b\n";
    let out = collect(src);
    assert_start(first(&out, "Code").span, src, "`code`", "Code");
}

#[test]
fn inline_latex_start() {
    let src = "a $x$ b\n";
    let out = collect(src);
    // span 应精确覆盖 `$x$`（含 `$`）。
    let item = first(&out, "Latex");
    let span = item.span.expect("Latex 应有 span");
    assert_eq!(
        &src[span.start..span.stop],
        "$x$",
        "Latex span 应覆盖 `$x$`"
    );
}

#[test]
fn inline_html_start() {
    let src = "a <b>x</b> c\n";
    let out = collect(src);
    // span 应精确覆盖 `<b>`（含尖括号）。
    let item = first(&out, "Html");
    let span = item.span.expect("Html 应有 span");
    assert_eq!(&src[span.start..span.stop], "<b>", "Html span 应覆盖 `<b>`");
}

// ---- 复合行内元素：span 可为 None，但若为 Some 则 start 精确 ----

#[test]
fn inline_compound_span_ok_or_none() {
    let src = "*em* **strong** ~~del~~ [link](url) ![img](i.png) <https://x.io>\n";
    let out = collect(src);
    for name in [
        "Emphasis",
        "Strong",
        "Strikethrough",
        "Link",
        "Image",
        "Autolink",
    ] {
        assert!(first(&out, name).span.is_none(), "{name} 当前应无 span");
    }
}

// ---- 综合场景：缩进/中文混合 ----

#[test]
fn mixed_indent_and_unicode() {
    let src = "  你好，**世界**！\n\n  > 引用\n\n  ---\n";
    let out = collect(src);
    // ThematicBreak 在缩进 `  ---` 时 start 指向 `-`。
    assert_start(
        first(&out, "ThematicBreak").span,
        src,
        "---",
        "ThematicBreak(缩进)",
    );
    // BlockQuote 内 Text "引用" 的 span 切片应精确等于内容。
    let found = out.iter().any(|i| {
        if i.name != "Text" {
            return false;
        }
        match i.span {
            Some(s) => &src[s.start..s.stop] == "引用",
            None => false,
        }
    });
    assert!(found, "应找到精确覆盖 `引用` 的 Text span: {out:?}");
}

// ---- 行内叶子 span start 精确性汇总 ----

#[test]
fn inline_leaf_span_start_precision() {
    let src = "x `code` $y$ <i>z</i> end\n";
    let out = collect(src);
    assert_start(first(&out, "Code").span, src, "`code`", "Code");
    assert_start(first(&out, "Latex").span, src, "$y$", "Latex");
    assert_start(first(&out, "Html").span, src, "<i>", "Html");
}

#[test]
fn strong_inner_cjk_span_precision() {
    // 复合节点（Strong）内 Text 的 span 应精确覆盖内容（多字节中文）。
    let src = "你好，**世界**！\n";
    let doc = tuack_ng_parser::parse(src);
    let mut texts: Vec<(String, Option<tuack_ng_parser::Span>)> = Vec::new();
    for block in &doc.blocks {
        if let BlockKind::Paragraph(inlines) = &block.value {
            for inline in inlines {
                if let InlineKind::Strong(children) = &inline.value {
                    for child in children {
                        if let InlineKind::Text(t) = &child.value {
                            texts.push((t.clone(), child.span));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(texts.len(), 1, "应 1 个 Strong 内文本，实际 {texts:?}");
    let (text, span) = &texts[0];
    assert_eq!(text, "世界");
    let span = span.expect("文本应有 span");
    // "你好，**世界**！"：你=0..3 好=3..6，=6..9 *=9..11 世=11..14 界=14..17
    assert_eq!(
        (span.start, span.stop),
        (11, 17),
        "span 应精确到字节 11..17"
    );
    assert_eq!(&src[span.start..span.stop], "世界");
}

#[test]
fn block_heading_span_covers_whole_line() {
    // Heading 的 span 应覆盖整行（start 到最后一个行内节点末尾），而非只到第一个节点。
    let src = "前文\n## 样例 1 输入\n后文\n";
    let out = collect(src);
    let item = first(&out, "Heading");
    let span = item.span.expect("Heading 应有 span");
    assert_eq!(&src[span.start..span.stop], "## 样例 1 输入");
}

#[test]
fn block_latex_span_covers_whole_block() {
    // LatexBlock 的 span 应覆盖整块（含两端 `$$`，不含结尾换行）。
    let src = "前文\n$$\n公式\n$$\n后文\n";
    let out = collect(src);
    let item = first(&out, "LatexBlock");
    let span = item.span.expect("LatexBlock 应有 span");
    assert_eq!(&src[span.start..span.stop], "$$\n公式\n$$");
}
