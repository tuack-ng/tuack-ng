//! 行内节点。

use crate::span::Spanned;

/// 行内构造。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineKind {
    /// 纯文本。
    Text(String),
    /// 软换行（行内换行，渲染为 `\n` 或空格）。
    SoftBreak,
    /// 硬换行。
    LineBreak,
    /// 行内代码。
    Code(String),
    /// LaTeX 公式。
    Latex(String),
    /// 原始 HTML 片段。
    Html(String),
    /// 链接（内联形式 `[text](url)`）。
    Link(Link),
    /// 引用式链接（`[text][label]` / `[text][]` / `[text]`）。
    LinkReference(LinkReference),
    /// 图片。
    Image(Image),
    /// 强调（`*` / `_`）。
    Emphasis(Vec<Inline>),
    /// 加粗（`**` / `__`）。
    Strong(Vec<Inline>),
    /// 删除线（`~~`）。
    Strikethrough(Vec<Inline>),
    /// 自动链接（`<https://>` 或 `<mailto:…>`）。
    Autolink(Autolink),
    /// 脚注引用（`[^label]`）。
    FootnoteReference(String),
    /// 空元素。
    Empty,
}

/// 行内节点别名（子节点均携带可选的 span）。
pub type Inline = Spanned<InlineKind>;

/// 链接种类：内联 / 引用 / 自动。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkKind {
    Inline,
    Reference(LinkReferenceKind),
    Auto,
}

/// 引用式链接的形式（roundtrip 保真用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkReferenceKind {
    /// `[text][label]`。
    Full,
    /// `[text][]`。
    Collapsed,
    /// `[text]`（label 即 text）。
    Shortcut,
}

/// 内联链接（`[text](url)`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub destination: String,
    pub title: Option<String>,
    pub children: Vec<Inline>,
}

/// 引用式链接（`[text][label]` 或 `[label][]`），destination 已在解析期解析。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkReference {
    pub destination: String,
    pub title: Option<String>,
    /// 引用 label（如 `[ref]` 中的 `ref`）。
    pub label: String,
    /// 链接显示文本。
    pub text: Vec<Inline>,
    /// 引用形式（roundtrip 保真）。
    pub kind: LinkReferenceKind,
}

/// 自动链接（`<https://>` 或 `<mailto:…>`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Autolink {
    pub url: String,
    /// 链接显示文本（`<...>` 内部）。
    pub text: String,
}

/// 图片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub destination: String,
    pub title: Option<String>,
    pub alt: String,
    pub attr: Option<ImageAttributes>,
}

/// 图片属性。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImageAttributes {
    pub width: Option<String>,
    pub height: Option<String>,
}
