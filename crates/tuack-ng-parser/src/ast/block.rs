//! 块级节点。

use super::inline::Inline;
use super::list::List;
use super::table::Table;
use crate::span::Spanned;

/// 块级构造。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    /// 普通段落。
    Paragraph(Vec<Inline>),
    /// ATX（`# Heading`）或 Setext（`===`）标题。
    Heading(Heading),
    /// 水平分隔线。
    ThematicBreak,
    /// 引用块。
    BlockQuote(Vec<Block>),
    /// 列表（无序或有序）。
    List(List),
    /// 围栏或缩进代码块。
    CodeBlock(CodeBlock),
    /// 原始 HTML 块。
    HtmlBlock(String),
    /// 链接引用定义。
    Definition(LinkDefinition),
    /// 表格。
    Table(Table),
    /// 脚注定义。
    FootnoteDefinition(FootnoteDefinition),
    /// 容器块（`:::{kind}`）。
    Container(Container),
    /// LaTeX 块。
    LatexBlock(String),
    /// 空块。
    Empty,
}

/// 块节点别名（嵌套子节点均携带可选的 span）。
pub type Block = Spanned<BlockKind>;

/// 容器块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub kind: String,
    pub params: Vec<ContainerParam>,
    pub blocks: Vec<Block>,
}

/// 容器参数。
///
/// 来源可为键值对（`:::{caption="标题"}`）或无值的裸属性（`:::{right}`）；
/// 裸属性按布尔标记处理，后续可扩展混合列表（`:::{aa, bb, b=c, c=d}`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerParam {
    /// 键值对，如 `caption="标题"`、`id="x"`。
    KeyValue(String, String),
    /// 无值单属性（布尔标记），如 `right`。
    Flag(String),
}

impl ContainerParam {
    /// 参数名（键）。
    pub fn key(&self) -> &str {
        match self {
            ContainerParam::KeyValue(k, _) => k,
            ContainerParam::Flag(k) => k,
        }
    }

    /// 参数值：键值对返回 `Some(值)`，裸属性返回 `None`。
    pub fn value(&self) -> Option<&str> {
        match self {
            ContainerParam::KeyValue(_, v) => Some(v),
            ContainerParam::Flag(_) => None,
        }
    }
}

/// 标题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    pub kind: HeadingKind,
    pub content: Vec<Inline>,
}

/// 标题种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadingKind {
    /// ATX 标题（`# Heading`）。
    Atx(u8),
    /// Setext 标题。
    Setext(SetextHeading),
}

/// Setext 标题种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetextHeading {
    /// `===` 下划线。
    Level1,
    /// `---` 下划线。
    Level2,
}

/// 代码块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlock {
    pub kind: CodeBlockKind,
    pub literal: String,
}

/// 代码块种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeBlockKind {
    Indented,
    Fenced { info: Option<String> },
}

/// 链接引用定义。
///
/// `label` 是纯文本标识符（可含 `*` 等字符但不解析行内语法），用于匹配引用链接。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkDefinition {
    pub label: String,
    pub destination: String,
    pub title: Option<String>,
}

/// 脚注定义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FootnoteDefinition {
    pub label: String,
    pub blocks: Vec<Block>,
}
