//! 脚注测试：块级定义 `[^label]: 内容` 与行内引用 `[^label]`。

mod common;

use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::printers::{render_markdown, render_typst};
use tuack_ng_parser::visitor::{VisitWith, Visitor};

#[test]
fn footnote_reference_and_definition() {
    // autocorrect-disable
    // 测试输入故意紧贴：GFM 脚注语法 `[^note]` 无需与中文分隔。
    let doc = tuack_ng_parser::parse("正文[^note]继续\n\n[^note]: 脚注内容\n");
    // autocorrect-enable
    // 行内引用
    let refs: Vec<String> = doc
        .blocks
        .iter()
        .filter_map(|b| match &b.value {
            BlockKind::Paragraph(inlines) => inlines.iter().find_map(|i| match &i.value {
                InlineKind::FootnoteReference(l) => Some(l.clone()),
                _ => None,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(refs, vec!["note".to_string()], "应识别行内引用");
    // 块级定义
    let def = doc.blocks.iter().find_map(|b| match &b.value {
        BlockKind::FootnoteDefinition(fd) => Some(fd),
        _ => None,
    });
    let def = def.expect("应有脚注定义");
    assert_eq!(def.label, "note");
    assert_eq!(def.blocks.len(), 1, "定义应含 1 块");
}

#[test]
fn footnote_span() {
    // autocorrect-disable
    // 测试输入故意紧贴：验证 span 精确覆盖 `[^note]`。
    let src = "正文[^note]继续\n\n[^note]: 脚注内容\n";
    // autocorrect-enable
    let doc = tuack_ng_parser::parse(src);
    struct V {
        ref_span: Option<(usize, usize)>,
        def_span: Option<(usize, usize)>,
    }
    impl Visitor for V {
        fn visit_inline(&mut self, i: &tuack_ng_parser::Inline) {
            if matches!(i.value, InlineKind::FootnoteReference(_)) {
                if let Some(s) = i.span {
                    self.ref_span = Some((s.start, s.stop));
                }
            }
            self.walk_inline(i);
        }
        fn visit_block(&mut self, b: &tuack_ng_parser::Block) {
            if matches!(b.value, BlockKind::FootnoteDefinition(_)) {
                if let Some(s) = b.span {
                    self.def_span = Some((s.start, s.stop));
                }
            }
            self.walk_block(b);
        }
    }
    let mut v = V {
        ref_span: None,
        def_span: None,
    };
    doc.visit_with(&mut v);
    // autocorrect-disable
    // `[^note]` 从第 4 字节起（正文[）覆盖到 `]` 后。
    // autocorrect-enable
    let (s, e) = v.ref_span.expect("引用应有 span");
    assert_eq!(&src[s..e], "[^note]", "引用 span 应覆盖 `[^note]`");
    // 定义块 start 指向 `[^note]:` 行首。
    let (s, _) = v.def_span.expect("定义应有 span");
    assert_eq!(&src[s..s + 4], "[^no", "定义 span 应从 `[^` 起");
}

#[test]
fn footnote_markdown_roundtrip() {
    // autocorrect-disable
    // 测试输入故意紧贴。
    let src = "正文[^note]继续\n\n[^note]: 脚注内容\n";
    // autocorrect-enable
    let doc = tuack_ng_parser::parse(src);
    let md = render_markdown(&doc);
    assert!(md.contains("[^note]"), "markdown 应保留引用，实际 {md:?}");
    assert!(
        md.contains("[^note]: 脚注内容"),
        "markdown 应保留定义，实际 {md:?}"
    );
    // roundtrip：重新解析应结构一致
    let doc2 = tuack_ng_parser::parse(&md);
    let mut b1 = doc.blocks;
    let mut b2 = doc2.blocks;
    common::strip_public_spans(&mut b1);
    common::strip_public_spans(&mut b2);
    assert_eq!(b1, b2, "往返应幂等");
}

#[test]
fn footnote_typst_inlines_definition() {
    // autocorrect-disable
    // 测试输入故意紧贴。
    let src = "正文[^note]继续\n\n[^note]: 脚注内容\n";
    // autocorrect-enable
    let doc = tuack_ng_parser::parse(src);
    let typ = render_typst(&doc);
    // 引用点应内联定义内容（而非 label）。
    assert!(
        typ.contains("#footnote[#\"脚注内容\"]"),
        "应内联定义内容，实际 {typ:?}"
    );
    assert!(
        !typ.contains("#footnote[note]"),
        "不应输出 label，实际 {typ:?}"
    );
}

#[test]
fn footnote_multiline_definition() {
    // autocorrect-disable
    // 测试输入故意紧贴。
    let src = "引用[^multi]\n\n[^multi]: 第一段\n    第二段\n";
    // autocorrect-enable
    let doc = tuack_ng_parser::parse(src);
    let def = doc.blocks.iter().find_map(|b| match &b.value {
        BlockKind::FootnoteDefinition(fd) => Some(fd),
        _ => None,
    });
    let def = def.expect("应有脚注定义");
    // 续行 `    第二段` 属同一段落（软换行），故为 1 块含 SoftBreak。
    assert_eq!(def.blocks.len(), 1, "应为 1 个段落，实际 {:?}", def.blocks);
    if let BlockKind::Paragraph(inlines) = &def.blocks[0].value {
        assert!(
            inlines
                .iter()
                .any(|i| matches!(i.value, InlineKind::SoftBreak)),
            "应含软换行，实际 {:?}",
            inlines
        );
    } else {
        panic!("定义内容应为段落");
    }
}

#[test]
fn footnote_reference_without_definition() {
    // 无定义的引用：typst 应输出 `[^label]` 原样。
    // autocorrect-disable
    // 测试输入故意紧贴。
    let src = "孤立引用[^orphan]\n";
    // autocorrect-enable
    let doc = tuack_ng_parser::parse(src);
    let typ = render_typst(&doc);
    assert!(
        typ.contains("[^orphan]"),
        "无定义引用应原样输出，实际 {typ:?}"
    );
}

#[test]
fn footnote_same_label_reused() {
    // 同一 label 多处引用 + 单一定义。
    let src = "a[^x] b[^x]\n\n[^x]: 内容\n";
    let doc = tuack_ng_parser::parse(src);
    let count = doc
        .blocks
        .iter()
        .filter_map(|b| match &b.value {
            BlockKind::Paragraph(inlines) => Some(
                inlines
                    .iter()
                    .filter(|i| matches!(i.value, InlineKind::FootnoteReference(_)))
                    .count(),
            ),
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(count, 2, "应识别 2 处引用");
    let defs = doc
        .blocks
        .iter()
        .filter(|b| matches!(b.value, BlockKind::FootnoteDefinition(_)))
        .count();
    assert_eq!(defs, 1, "应只有 1 个定义");
}
