//! 脚注扩展：块级定义 `[^label]: 内容` 与行内引用 `[^label]`。
//!
//! 借鉴 rushdown-footnote 的解析逻辑，但简化：
//! 只保留 label 与内容，不携带自动编号（index/ref_index）——那些是
//! rushdown HTML 渲染器需要的，本项目渲染（markdown/typst）由 printer 处理。

use core::fmt;

use rushdown::ast::{Arena, KindData, NodeRef, NodeType, PrettyPrint};
use rushdown::parser::{
    AnyBlockParser, AnyInlineParser, BlockParser, Context, InlineParser, NoParserOptions,
    PRIORITY_LINK, PRIORITY_LIST, Parser, ParserExtension, ParserExtensionFn, State,
};
use rushdown::text::{self, Reader as _};

/// 行内脚注引用节点 `[^label]`。
#[derive(Debug)]
pub struct FootnoteReferenceNode {
    pub label: String,
    /// 源码字节区间（覆盖 `[^label]` 整体，不含 `!`）。
    pub start: usize,
    pub stop: usize,
}

impl FootnoteReferenceNode {
    fn new(label: String, start: usize, stop: usize) -> Self {
        Self { label, start, stop }
    }
}

impl rushdown::ast::NodeKind for FootnoteReferenceNode {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }
    fn kind_name(&self) -> &'static str {
        "FootnoteReference"
    }
}

impl PrettyPrint for FootnoteReferenceNode {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, _level: usize) -> fmt::Result {
        writeln!(w, "FootnoteReference label={:?}", self.label)
    }
}

impl From<FootnoteReferenceNode> for KindData {
    fn from(d: FootnoteReferenceNode) -> Self {
        KindData::Extension(Box::new(d))
    }
}

/// 块级脚注定义节点 `[^label]: 内容`。
#[derive(Debug)]
pub struct FootnoteDefinitionNode {
    pub label: String,
}

impl FootnoteDefinitionNode {
    fn new(label: String) -> Self {
        Self { label }
    }
}

impl rushdown::ast::NodeKind for FootnoteDefinitionNode {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }
    fn kind_name(&self) -> &'static str {
        "FootnoteDefinition"
    }
}

impl PrettyPrint for FootnoteDefinitionNode {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, _level: usize) -> fmt::Result {
        writeln!(w, "FootnoteDefinition label={:?}", self.label)
    }
}

impl From<FootnoteDefinitionNode> for KindData {
    fn from(d: FootnoteDefinitionNode) -> Self {
        KindData::Extension(Box::new(d))
    }
}

/// 块级脚注定义解析器：`[^label]: 内容`（内容为块级）。
#[derive(Debug, Default)]
pub struct FootnoteDefinitionParser {}

impl FootnoteDefinitionParser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlockParser for FootnoteDefinitionParser {
    fn trigger(&self) -> &[u8] {
        b"["
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BasicReader,
        ctx: &mut Context,
    ) -> Option<(NodeRef, State)> {
        let (line, seg) = reader.peek_line_bytes()?;
        let mut pos = ctx.block_offset()?;
        pos += 1; // skip '['
        if !line.get(pos)?.eq(&b'^') {
            return None;
        }
        let open = pos + 1;
        let mut cur = open;
        let mut close = 0usize;
        while cur < line.len() {
            let c = line[cur];
            if c == b'\\' && line.get(cur + 1) == Some(&b']') {
                cur += 2;
                continue;
            }
            if c == b']' {
                close = cur;
                break;
            }
            cur += 1;
        }
        if close == 0 {
            return None;
        }
        if line.get(close + 1) != Some(&b':') {
            return None;
        }

        let label = text::Segment::new(
            seg.start() + open - seg.padding(),
            seg.start() + close - seg.padding(),
        );
        if label.is_blank(reader.source()) {
            return None;
        }

        let node = arena.new_node(FootnoteDefinitionNode::new(
            label.str(reader.source()).into_owned(),
        ));
        reader.advance(close + 2);
        Some((node, State::HAS_CHILDREN))
    }

    fn cont(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut Context,
    ) -> Option<State> {
        let (line, _) = reader.peek_line_bytes()?;
        if rushdown::util::is_blank(&line) {
            return Some(State::HAS_CHILDREN);
        }
        let (childpos, padding) = rushdown::util::indent_position(&line, reader.line_offset(), 4)?;
        reader.advance_and_set_padding(childpos, padding);
        Some(State::HAS_CHILDREN)
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

impl From<FootnoteDefinitionParser> for AnyBlockParser {
    fn from(p: FootnoteDefinitionParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

/// 行内脚注引用解析器：`[^label]`。
#[derive(Debug, Default)]
pub struct FootnoteReferenceParser {}

impl FootnoteReferenceParser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl InlineParser for FootnoteReferenceParser {
    fn trigger(&self) -> &[u8] {
        // 与图片语法 `![` 冲突规避：用 `!` 触发，实际语法仍为 `[^label]`。
        b"!["
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BlockReader,
        _ctx: &mut Context,
    ) -> Option<NodeRef> {
        let (line, seg) = reader.peek_line_bytes()?;
        let mut pos = 1;
        if line.first() == Some(&b'!') {
            pos += 1;
        }
        if line.get(pos)? != &b'^' {
            return None;
        }
        let open = pos + 1;
        let mut cur = open;
        let mut close = 0usize;
        while cur < line.len() {
            let c = line[cur];
            if c == b'\\' && line.get(cur + 1) == Some(&b']') {
                cur += 2;
                continue;
            }
            if c == b']' {
                close = cur;
                break;
            }
            cur += 1;
        }
        if close == 0 {
            return None;
        }

        let start = seg.start() + open - 2; // 覆盖 `[^label]`（从 `[` 起）
        let stop = seg.start() + close + 1; // 到 `]` 后
        let label = reader.source()[seg.start() + open..seg.start() + close].to_string();
        let node = arena.new_node(FootnoteReferenceNode::new(label, start, stop));
        reader.advance(close + 1);
        if line[0] == b'!' {
            _parent_ref.merge_or_append_text(arena, (seg.start(), seg.start() + 1).into());
        }
        Some(node)
    }
}

impl From<FootnoteReferenceParser> for AnyInlineParser {
    fn from(p: FootnoteReferenceParser) -> Self {
        AnyInlineParser::Extension(Box::new(p))
    }
}

/// 构造脚注扩展。
pub fn footnote_parser_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_inline_parser(
            FootnoteReferenceParser::new,
            NoParserOptions,
            PRIORITY_LINK - 100,
        );
        p.add_block_parser(
            FootnoteDefinitionParser::new,
            NoParserOptions,
            PRIORITY_LIST + 100,
        );
    })
}
