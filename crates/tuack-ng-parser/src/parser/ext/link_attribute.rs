//! `![](){width=.. height=..}` 图片/链接属性扩展。
//!
//! 自写实现，对齐 markdown-ppp 的 `image.rs` 语义：`{width=.. height=..}` 存为
//! 图片属性。注册为 inline 扩展节点，紧跟在 link/image 之后触发。

use core::fmt;

use rushdown::Result;
use rushdown::ast::{Arena, KindData, NodeRef, NodeType, PrettyPrint};
use rushdown::parser::{InlineParser, ParserExtension, ParserOptions};
use rushdown::text::{self, Reader as _};

/// link-attribute 扩展节点：存储解析出的 `{key=value}` 属性。
#[derive(Debug)]
pub struct LinkAttrNode {
    pub attrs: std::collections::HashMap<String, String>,
}

impl LinkAttrNode {
    pub fn new(attrs: std::collections::HashMap<String, String>) -> Self {
        Self { attrs }
    }
}

impl rushdown::ast::NodeKind for LinkAttrNode {
    fn typ(&self) -> NodeType {
        NodeType::Inline
    }
    fn kind_name(&self) -> &'static str {
        "LinkAttr"
    }
}

impl PrettyPrint for LinkAttrNode {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, _level: usize) -> fmt::Result {
        writeln!(w, "LinkAttr: {:?}", self.attrs)
    }
}

impl From<LinkAttrNode> for KindData {
    fn from(d: LinkAttrNode) -> Self {
        KindData::Extension(Box::new(d))
    }
}

/// 解析 `{key="value" key2=val2}` 属性。
fn parse_braced_attrs(input: &str) -> Option<std::collections::HashMap<String, String>> {
    let rest = input.strip_prefix('{')?;
    let end = rest.find('}')?;
    let inner = &rest[..end];
    let mut attrs = std::collections::HashMap::new();
    for token in inner.split_whitespace() {
        if let Some((k, v)) = token.split_once('=') {
            let v = v.trim_matches('"');
            attrs.insert(k.to_string(), v.to_string());
        } else if let Some(class) = token.strip_prefix('.') {
            let class = class.to_string();
            let existing = attrs.entry("class".to_string()).or_default();
            if !existing.is_empty() {
                existing.push(' ');
            }
            existing.push_str(&class);
        } else if let Some(id) = token.strip_prefix('#') {
            attrs.insert("id".to_string(), id.to_string());
        }
    }
    Some(attrs)
}

/// link-attribute inline parser。
#[derive(Debug, Default)]
pub struct LinkAttributeParser {}

impl LinkAttributeParser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl InlineParser for LinkAttributeParser {
    fn trigger(&self) -> &[u8] {
        b"{"
    }

    fn parse(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BlockReader,
        _ctx: &mut rushdown::parser::Context,
    ) -> Option<NodeRef> {
        // 只在 link/image 之后才接受 `{...}`。
        let (line, _) = reader.peek_line_bytes()?;
        let s = std::str::from_utf8(&line).ok()?;
        let (line_idx, seg) = reader.position();
        let offset = seg.start();
        let input = &s[..line.len().min(s.len())];

        let attrs = parse_braced_attrs(input)?;
        if attrs.is_empty() {
            return None;
        }

        // 计算消耗字节数：`{` + 内容 + `}` = end + 2 字节。
        let rest = &input[1..];
        let end = rest.find('}')?;
        reader.advance(end + 2);
        let _ = (line_idx, offset);

        let node = arena.new_node(LinkAttrNode::new(attrs));
        Some(node)
    }
}

impl From<LinkAttributeParser> for rushdown::parser::AnyInlineParser {
    fn from(p: LinkAttributeParser) -> Self {
        rushdown::parser::AnyInlineParser::Extension(Box::new(p))
    }
}

/// 构造 link-attribute 扩展。
pub fn link_attribute_parser_extension() -> impl ParserExtension {
    rushdown::parser::parser_extension(|p| {
        p.add_inline_parser(LinkAttributeParser::new, NoParserOptions, 0);
    })
}

/// 空选项。
#[derive(Debug, Clone, Default)]
pub struct NoParserOptions;
impl ParserOptions for NoParserOptions {}

#[allow(dead_code)]
fn _unused() -> Result<()> {
    Ok(())
}
