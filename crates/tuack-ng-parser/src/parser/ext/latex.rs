//! LaTeX 公式扩展：行内 `$...$` 与块级 `$$...$$`。
//!
//! 自写实现，rushdown 本身没有 math/latex 解析器。

use core::fmt;

use rushdown::ast::{Arena, KindData, NodeRef, NodeType, PrettyPrint};
use rushdown::parser::{
    AnyBlockParser, AnyInlineParser, BlockParser, Context, InlineParser, ParserExtension,
    ParserOptions, State,
};
use rushdown::text::{self, Reader as _};

/// 行内 LaTeX 扩展节点。
#[derive(Debug)]
pub struct LatexNode {
    pub content: String,
    /// 源码字节区间（含两端 `$`）。
    pub start: usize,
    pub stop: usize,
}

impl LatexNode {
    pub fn new(content: String, start: usize, stop: usize) -> Self {
        Self {
            content,
            start,
            stop,
        }
    }
}

impl rushdown::ast::NodeKind for LatexNode {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }
    fn kind_name(&self) -> &'static str {
        "Latex"
    }
}

impl PrettyPrint for LatexNode {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, _level: usize) -> fmt::Result {
        writeln!(w, "Latex content={:?}", self.content)
    }
}

impl From<LatexNode> for KindData {
    fn from(d: LatexNode) -> Self {
        KindData::Extension(Box::new(d))
    }
}

/// 块级 LaTeX 扩展节点（`$$...$$` 跨行公式）。`start`/`stop` 覆盖整块（含两端 `$$`）。
#[derive(Debug)]
pub struct LatexBlockNode {
    pub content: String,
    pub start: usize,
    pub stop: usize,
}

impl LatexBlockNode {
    fn new() -> Self {
        Self {
            content: String::new(),
            start: 0,
            stop: 0,
        }
    }
}

impl rushdown::ast::NodeKind for LatexBlockNode {
    fn typ(&self) -> NodeType {
        NodeType::LeafBlock
    }
    fn kind_name(&self) -> &'static str {
        "LatexBlock"
    }
}

impl PrettyPrint for LatexBlockNode {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, _level: usize) -> fmt::Result {
        writeln!(w, "LatexBlock content={:?}", self.content)
    }
}

impl From<LatexBlockNode> for KindData {
    fn from(d: LatexBlockNode) -> Self {
        KindData::Extension(Box::new(d))
    }
}

/// 行内 LaTeX 解析器：`$...$`（单 `$`）。
#[derive(Debug, Default)]
pub struct LatexParser {}

impl LatexParser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl InlineParser for LatexParser {
    fn trigger(&self) -> &[u8] {
        b"$"
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BlockReader,
        _ctx: &mut Context,
    ) -> Option<NodeRef> {
        let (_line_no, seg) = reader.position();
        let pos = seg.start();

        let (line, _seg) = reader.peek_line_bytes()?;
        // 行内只认单 `$`：开头是 `$$` 则拒绝。
        if line.first() == Some(&b'$') && line.get(1) == Some(&b'$') {
            return None;
        }
        let start = 1;

        // 在同一行内找闭合 `$`。
        let rest = &line[start..];
        let content_end = rest.iter().position(|&b| b == b'$')?;
        // 闭合 `$$` 也拒绝（如 `$$x$$` 的尾部）。
        if rest.get(content_end + 1) == Some(&b'$') {
            return None;
        }
        let content = rest[..content_end].to_vec();

        reader.advance(start + content_end + 1);

        // 转成 UTF-8 字符串（可能含非 ASCII，尽量保留）。
        let content = String::from_utf8_lossy(&content).into_owned();
        // span 覆盖 `$...$`：pos 起，`$`+内容+`$`。
        let stop = pos + content_end + 2;
        let node = arena.new_node(LatexNode::new(content, pos, stop));
        arena[node].set_pos(pos);
        Some(node)
    }
}

/// 在字节序列中查找子序列位置。
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

impl From<LatexParser> for AnyInlineParser {
    fn from(p: LatexParser) -> Self {
        AnyInlineParser::Extension(Box::new(p))
    }
}

/// 块级 LaTeX 解析器：`$$...$$`（跨行公式）。
#[derive(Debug, Default)]
pub struct LatexBlockParser {}

impl LatexBlockParser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlockParser for LatexBlockParser {
    fn trigger(&self) -> &[u8] {
        b"$"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut Context,
    ) -> Option<(NodeRef, State)> {
        let (line, _seg) = reader.peek_line_bytes()?;
        let trimmed = trim_ascii_space(line.as_ref());
        // 行首必须是 `$$`。
        if !trimmed.starts_with(b"$$") {
            return None;
        }
        // 同行闭合 `$$...$$` 不识别为块级（保持原文，由行内/文本处理）。
        let rest = &trimmed[2..];
        if find_subslice(rest, b"$$").is_some() {
            return None;
        }
        // 跨行公式：跳过开头 `$$`，内容由 cont 累积。
        let start = reader.position().1.start();
        let node = arena.new_node(LatexBlockNode::new());
        arena[node].set_pos(start);
        rushdown::as_extension_data_mut!(arena, node, LatexBlockNode).start = start;
        reader.advance_to_eol();
        Some((node, State::NO_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut Context,
    ) -> Option<State> {
        let (line, _seg) = reader.peek_line_bytes()?;
        let trimmed = trim_ascii_space(line.as_ref());
        // 闭合 fence：以 `$$` 开头。
        if trimmed.starts_with(b"$$") {
            let (_, seg) = reader.position();
            let mut stop = seg.stop();
            if reader.source().as_bytes().get(stop.wrapping_sub(1)) == Some(&b'\n') {
                stop -= 1;
            }
            rushdown::as_extension_data_mut!(arena, node_ref, LatexBlockNode).stop = stop;
            reader.advance_to_eol();
            return None;
        }
        // 累积内容行（保留原始行）。
        let seg = reader.peek_line_segment().unwrap();
        rushdown::as_type_data_mut!(arena, node_ref, Block).append_source_line(seg);
        reader.advance_to_eol();
        Some(State::NO_CHILDREN)
    }

    fn close(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut text::BasicReader,
        _ctx: &mut Context,
    ) {
        let lines = rushdown::as_type_data_mut!(arena, node_ref, Block).take_source();
        let content = lines
            .iter()
            .map(|seg| seg.str(reader.source()).into_owned())
            .collect::<String>();
        rushdown::as_extension_data_mut!(arena, node_ref, LatexBlockNode).content = content;
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

impl From<LatexBlockParser> for AnyBlockParser {
    fn from(p: LatexBlockParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

/// 构造块级 LaTeX 扩展。
pub fn latex_block_parser_extension() -> impl ParserExtension {
    rushdown::parser::parser_extension(|p| {
        p.add_block_parser(LatexBlockParser::new, NoParserOptions, 700);
    })
}

fn trim_ascii_space(b: &[u8]) -> &[u8] {
    let start = b
        .iter()
        .position(|&c| c != b' ' && c != b'\t')
        .unwrap_or(b.len());
    &b[start..]
}

/// 构造行内 LaTeX 扩展。
pub fn latex_parser_extension() -> impl ParserExtension {
    rushdown::parser::parser_extension(|p| {
        p.add_inline_parser(LatexParser::new, NoParserOptions, 1000);
    })
}

/// 空选项。
#[derive(Debug, Clone, Default)]
pub struct NoParserOptions;
impl ParserOptions for NoParserOptions {}

#[allow(dead_code)]
fn _unused() -> Option<NodeRef> {
    None
}
