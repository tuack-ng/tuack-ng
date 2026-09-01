//! 框架设施测试：transform（改写）与 visitor（遍历）。

use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::transform::Transform;
use tuack_ng_parser::visitor::{VisitWith, Visitor};

// ---- transform ----

#[test]
fn transform_link_urls() {
    let mut doc = tuack_ng_parser::parse("[a](http://example.com)");
    doc.transform_link_urls(|url| url.replace("http://", "https://"));
    let mut found = false;
    for block in &doc.blocks {
        if let BlockKind::Paragraph(inlines) = &block.value {
            for inline in inlines {
                if let InlineKind::Link(l) = &inline.value {
                    assert_eq!(l.destination, "https://example.com");
                    found = true;
                }
            }
        }
    }
    assert!(found);
}

#[test]
fn transform_map_blocks() {
    let mut doc = tuack_ng_parser::parse("a\n\nb");
    doc.map_blocks(|b| b);
    assert_eq!(doc.blocks.len(), 2);
}

#[test]
fn transform_image_urls() {
    let src = "![图](img/a.png)\n";
    let mut doc = tuack_ng_parser::parse(src);
    doc.transform_image_urls(|url| url.replace("img/", "cdn/"));
    let mut found = false;
    for block in &doc.blocks {
        if let BlockKind::Paragraph(inlines) = &block.value {
            for inline in inlines {
                if let InlineKind::Image(img) = &inline.value {
                    assert_eq!(img.destination, "cdn/a.png");
                    found = true;
                }
            }
        }
    }
    assert!(found, "应找到图片");
}

// ---- visitor ----

#[test]
fn visitor_heading_and_table() {
    let doc = tuack_ng_parser::parse("# 标题\n\n| a |\n| - |\n| b |");
    let mut counts = (0usize, 0usize); // (heading, table)
    struct V<'a>(&'a mut (usize, usize));
    impl Visitor for V<'_> {
        fn visit_block(&mut self, block: &tuack_ng_parser::ast::Block) {
            if let BlockKind::Heading(_) = &block.value {
                self.0.0 += 1;
            }
            if let BlockKind::Table(_) = &block.value {
                self.0.1 += 1;
            }
            self.walk_block(block);
        }
    }
    let mut visitor = V(&mut counts);
    doc.visit_with(&mut visitor);
    assert_eq!(counts.0, 1, "应访问到 1 个标题");
    assert_eq!(counts.1, 1, "应访问到 1 个表格");
}

#[test]
fn visitor_collect_text() {
    let doc = tuack_ng_parser::parse("Hello **world**\n");
    let mut visitor = Collect { texts: Vec::new() };
    doc.visit_with(&mut visitor);
    assert!(
        visitor.texts.contains(&"world".to_string()),
        "visitor 应收集到 world，实际 {:?}",
        visitor.texts
    );
}

struct Collect {
    texts: Vec<String>,
}
impl Visitor for Collect {
    fn visit_inline(&mut self, inline: &tuack_ng_parser::ast::Inline) {
        if let InlineKind::Text(t) = &inline.value {
            self.texts.push(t.clone());
        }
        self.walk_inline(inline);
    }
}
