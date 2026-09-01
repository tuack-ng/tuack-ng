//! `:::{kind}` fenced-div 容器块扩展。
//!
//! 借鉴 rushdown-fenced-div 的实现：用 rushdown 内置的 `parse_attributes` 解析
//! `{...}` 属性、用 depth 计数器支持嵌套；对齐 markdown-ppp 的 `container.rs`
//! 语义（kind + params）。

use core::fmt;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;

use rushdown::context::{ContextKey, ContextKeyRegistry, UsizeValue};
use rushdown::parser::{
    AnyBlockParser, BlockParser, Context, NoParserOptions, PRIORITY_LIST, Parser, ParserExtension,
    ParserExtensionFn, State,
};
use rushdown::text::{self, BlockReader, EOS, Reader as _};
use rushdown::util::{is_punct, is_space, resolve_entity_references, resolve_numeric_references};
use rushdown::{
    Result,
    ast::{Arena, Attributes, KindData, NodeRef, NodeType, PrettyPrint},
};

const OPEN_DIV_DEPTH: &str = "tuack-ng-parser-fenced-div-depth";

/// fenced-div 扩展节点：记录嵌套深度，kind/params 存于 attributes。
#[derive(Debug)]
pub struct FencedDiv {
    depth: usize,
}

impl FencedDiv {
    fn new(depth: usize) -> Self {
        Self { depth }
    }
}

impl rushdown::ast::NodeKind for FencedDiv {
    fn typ(&self) -> NodeType {
        NodeType::ContainerBlock
    }
    fn kind_name(&self) -> &'static str {
        "FencedDiv"
    }
}

impl PrettyPrint for FencedDiv {
    fn pretty_print(&self, w: &mut dyn fmt::Write, _source: &str, level: usize) -> fmt::Result {
        writeln!(
            w,
            "{}FencedDiv depth={}",
            rushdown::ast::pp_indent(level),
            self.depth
        )
    }
}

impl From<FencedDiv> for KindData {
    fn from(e: FencedDiv) -> Self {
        KindData::Extension(Box::new(e))
    }
}

/// fenced-div 块级解析器。
#[derive(Debug)]
pub struct FencedDivBlockParser {
    open_div_depth: ContextKey<UsizeValue>,
}

impl FencedDivBlockParser {
    pub fn new(reg: Rc<RefCell<ContextKeyRegistry>>) -> Self {
        let open_div_depth = reg.borrow_mut().get_or_create::<UsizeValue>(OPEN_DIV_DEPTH);
        Self { open_div_depth }
    }
}

impl BlockParser for FencedDivBlockParser {
    fn trigger(&self) -> &[u8] {
        b":"
    }

    fn open(
        &self,
        arena: &mut Arena,
        _parent_ref: NodeRef,
        reader: &mut text::BasicReader,
        ctx: &mut Context,
    ) -> Option<(NodeRef, State)> {
        let segment = reader.peek_line_segment()?;
        let blk = [segment];
        let mut blk_reader = BlockReader::new(reader.source(), &blk);
        let fence_length = blk_reader.skip_while(|b| b == b':');
        if fence_length < 3 {
            return None;
        }
        let depth = ctx.get(self.open_div_depth).copied().unwrap_or(0) + 1;
        let node_ref = parse_opening_fence(arena, &mut blk_reader, depth)?;
        ctx.insert(self.open_div_depth, depth);
        reader.advance_to_eol();
        Some((node_ref, State::HAS_CHILDREN))
    }

    fn cont(
        &self,
        arena: &mut Arena,
        node_ref: NodeRef,
        reader: &mut text::BasicReader,
        ctx: &mut Context,
    ) -> Option<State> {
        if let Some(last_opened_block) = ctx.last_opened_block() {
            if last_opened_block != node_ref
                && matches!(arena[last_opened_block].kind_data(), KindData::CodeBlock(_))
            {
                return Some(State::HAS_CHILDREN);
            }
        }
        let (line, _) = reader.peek_line_bytes()?;
        let fence_length = line.iter().take_while(|&&b| b == b':').count();
        if fence_length < 3 {
            return Some(State::HAS_CHILDREN);
        }
        let rest = &line[fence_length..];
        if rest
            .iter()
            .take_while(|&&b| b.is_ascii_whitespace())
            .count()
            < rest.len()
        {
            return Some(State::HAS_CHILDREN);
        }
        let fenced_div = rushdown::as_extension_data!(arena, node_ref, FencedDiv);
        let open_depth = ctx.get(self.open_div_depth).copied().unwrap_or(0);
        if fenced_div.depth == open_depth {
            reader.advance_to_eol();
            return None;
        }
        Some(State::HAS_CHILDREN)
    }

    fn close(
        &self,
        _arena: &mut Arena,
        _node_ref: NodeRef,
        _reader: &mut text::BasicReader,
        ctx: &mut Context,
    ) {
        if let Some(depth) = ctx.get_mut(self.open_div_depth) {
            *depth = depth.saturating_sub(1);
        }
    }

    fn can_interrupt_paragraph(&self) -> bool {
        true
    }
}

/// 解析开头的 `::: {kind} {key=val}`。
fn parse_opening_fence(
    arena: &mut Arena,
    reader: &mut BlockReader,
    depth: usize,
) -> Option<NodeRef> {
    reader.skip_spaces();
    let b = reader.peek_byte();
    if b == EOS {
        return None;
    }
    let mut attributes = if b == b'{' {
        parse_brace_attributes(reader)?
    } else {
        // 无括号形式：`:::note` -> 视为 class。
        let (line, seg) = reader.peek_line_bytes()?;
        let i = line
            .iter()
            .take_while(|&&b| {
                !is_space(b) && (!is_punct(b) || b == b'_' || b == b'-' || b == b':' || b == b'.')
            })
            .count();
        if i == 0 {
            return None;
        }
        let mut attributes = Attributes::new();
        attributes.insert("class", seg.with_stop(seg.start() + i).into());
        reader.advance(i);
        attributes
    };
    reader.skip_spaces();
    // kind 后跟 `{key=val}`：追加解析属性（markdown-ppp 风格 `:::figure{caption=..}`，
    // 也支持无值裸参数，如 `:::align{right}`）。
    if reader.peek_byte() == b'{' {
        if let Some(extra) = parse_brace_attributes(reader) {
            attributes.extend(extra);
        }
    }
    reader.skip_spaces();
    reader.skip_while(|b| b == b':');
    reader.skip_spaces();
    if reader.peek_byte() != EOS {
        return None;
    }
    let node_ref = arena.new_node(FencedDiv::new(depth));
    arena[node_ref].attributes_mut().extend(attributes);
    Some(node_ref)
}

/// 解析 `{...}` 属性块。
///
/// 在 rushdown `parse_attributes` 支持的 `#id`、`.class`、`key="value"`、`key=value`
/// 之外，额外支持无值的裸参数（如 `:::align{right}`）。裸参数解析为
/// `MultilineValue::Empty`，之后在 `fenced_div_to_container` 中转成 `ContainerParam::Flag`。
///
/// 结构上与 rushdown `parse_attributes` 对齐：单属性解析失败时回滚 reader 位置再返回
/// `None`，保证其为 `parse_attributes` 的超集（空 `#id`/`.class` 简写同样被接受）。
fn parse_brace_attributes(reader: &mut BlockReader) -> Option<Attributes> {
    let (saved_line, saved_position) = reader.position();
    reader.skip_spaces();
    if reader.peek_byte() != b'{' {
        reader.set_position(saved_line, saved_position);
        return None;
    }
    reader.advance(1);
    let mut attrs = Attributes::new();
    loop {
        reader.skip_spaces();
        if reader.peek_byte() == b'}' {
            reader.advance(1);
            return Some(attrs);
        }
        let (name, value) = match parse_one_attribute(reader) {
            Some(pair) => pair,
            None => {
                // 与 rushdown 一致：失败回滚 reader 位置。
                reader.set_position(saved_line, saved_position);
                return None;
            }
        };
        if name == "class" && attrs.contains_key("class") {
            let s = String::from(attrs.get("class").unwrap().str(reader.source()));
            attrs.insert(name, (s + " " + &value.str(reader.source())).into());
        } else {
            attrs.insert(name, value);
        }
        reader.skip_spaces();
        if reader.peek_byte() == b',' {
            reader.advance(1);
        }
    }
}

/// 解析单个属性：`#id` / `.class` 简写、`key=value`，以及 tuack-ng 扩展的裸 `key`。
///
/// 对应 rushdown `parse_attribute`。失败返回 `None`，不回滚 reader 位置。
fn parse_one_attribute(reader: &mut BlockReader) -> Option<(String, text::MultilineValue)> {
    let (line, _seg) = reader.peek_line_bytes()?;
    if line.is_empty() {
        return None;
    }
    let first = line[0];
    if first == b'#' || first == b'.' {
        // `#id` / `.class` 简写；与 rushdown 一致，允许空名（`{#}` -> id=""、`{.}` -> class=""）。
        reader.advance(1);
        let (line, seg) = reader.peek_line_bytes()?;
        let i = line
            .iter()
            .take_while(|&&b| {
                !is_space(b) && (!is_punct(b) || b == b'_' || b == b'-' || b == b':' || b == b'.')
            })
            .count();
        reader.advance(i);
        let value = seg.with_stop(seg.start() + i).into();
        let name = if first == b'#' { "id" } else { "class" };
        Some((name.to_string(), value))
    } else {
        // `key=value` 或裸 `key`（空值）。
        if !(first.is_ascii_alphabetic() || first == b'_' || first == b':') {
            return None;
        }
        let i = line
            .iter()
            .take_while(|&&b| {
                b.is_ascii_alphabetic()
                    || b.is_ascii_digit()
                    || b == b'_'
                    || b == b'-'
                    || b == b':'
                    || b == b'.'
            })
            .count();
        let name = String::from_utf8_lossy(&line[..i]).into_owned();
        reader.advance(i);
        reader.skip_spaces();
        if reader.peek_byte() == b'=' {
            reader.advance(1);
            let value = parse_attr_value(reader)?;
            Some((name, value))
        } else {
            Some((name, text::MultilineValue::Empty))
        }
    }
}

/// 解析单个属性值：`"quoted"`、`'quoted'` 或未加引号的 `unquoted`。
fn parse_attr_value(reader: &mut BlockReader) -> Option<text::MultilineValue> {
    reader.skip_spaces();
    let value = match reader.peek_byte() {
        b'"' | b'\'' => {
            let quote = reader.peek_byte();
            reader.advance(1);
            let (line, seg) = reader.peek_line_bytes()?;
            let i = line.iter().position(|&b| b == quote)?;
            reader.advance(i + 1);
            seg.with_stop(seg.start() + i).into()
        }
        _ => {
            let (line, seg) = reader.peek_line_bytes()?;
            let i = line
                .iter()
                .take_while(|&&b| {
                    !is_space(b)
                        && b != b'}'
                        && b != b'"'
                        && b != b'\''
                        && b != b'='
                        && b != b'<'
                        && b != b'>'
                        && b != b'`'
                        && b != b','
                })
                .count();
            if i == 0 {
                return None;
            }
            reader.advance(i);
            seg.with_stop(seg.start() + i).into()
        }
    };
    Some(resolve_attr_entities(value, reader.source()))
}

/// 解析属性值中的 HTML 实体与数字引用（`&amp;` -> `&`、`&#35;` -> `#` 等），
/// 与 rushdown 内置 `parse_attributes` 的行为保持一致。
fn resolve_attr_entities(value: text::MultilineValue, source: &str) -> text::MultilineValue {
    let resolved = resolve_numeric_references(resolve_entity_references(value.bytes(source)));
    match resolved {
        Cow::Borrowed(_) => value,
        Cow::Owned(s) => s.into(),
    }
}

impl From<FencedDivBlockParser> for AnyBlockParser {
    fn from(p: FencedDivBlockParser) -> Self {
        AnyBlockParser::Extension(Box::new(p))
    }
}

/// 返回 fenced-div 解析扩展。
pub fn fenced_div_parser_extension() -> impl ParserExtension {
    ParserExtensionFn::new(|p: &mut Parser| {
        p.add_block_parser(
            FencedDivBlockParser::new,
            NoParserOptions,
            PRIORITY_LIST + 100,
        );
    })
}

/// 从扩展节点提取容器数据（kind + params）。
///
/// kind 取 `class` 属性（`:::note` / `:::{.note}`），其余属性作为 params。
/// 有值属性转 `ContainerParam::KeyValue`，无值裸属性转 `ContainerParam::Flag`。
pub(crate) fn fenced_div_to_container(
    attrs: &Attributes,
    source: &str,
) -> (String, Vec<crate::ast::ContainerParam>) {
    let mut kind = String::new();
    let mut params = Vec::new();
    for (k, v) in attrs.iter() {
        let val = v.str(source).into_owned();
        if k == "class" {
            kind = val;
        } else if matches!(v, text::MultilineValue::Empty) {
            // 无值裸属性 -> Flag；`key=""` 解析为 Indices（非 Empty），仍保留为空值 KeyValue。
            params.push(crate::ast::ContainerParam::Flag(k.to_string()));
        } else {
            params.push(crate::ast::ContainerParam::KeyValue(k.to_string(), val));
        }
    }
    (kind, params)
}

#[allow(dead_code)]
fn _unused() -> Result<()> {
    Ok(())
}
