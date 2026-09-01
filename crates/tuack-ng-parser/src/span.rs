//! 源码位置与 span 包装。

/// 源码中的字节区间 `[start, stop)`。
///
/// - 叶子 inline（Text/Code/Autolink/Latex/Html/LineBreak）：`source[start..stop]` 精确等于内容。
/// - Block 及复合 inline（Emphasis/Strong/Strikethrough）：不保证精确，仅作定位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub stop: usize,
}

impl Span {
    pub fn new(start: usize, stop: usize) -> Self {
        Self { start, stop }
    }

    /// 从源码切片。
    pub fn str<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.stop]
    }
}

impl From<(usize, usize)> for Span {
    fn from((start, stop): (usize, usize)) -> Self {
        Self { start, stop }
    }
}

/// 值 + 可选的源码位置。
///
/// 复合节点（Emphasis/Strong/Strikethrough 及 Block）不携带 span，为 `None`；
/// 叶子节点携带精确的 `Some(Span)`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Option<Span>,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: Option<Span>) -> Self {
        Self { value, span }
    }

    /// 带必现 span 的构造（`spanned` 方法因与类型同名被弃用，见 `same_name_method`）
    pub fn with_span(value: T, span: Span) -> Self {
        Self {
            value,
            span: Some(span),
        }
    }

    pub fn plain(value: T) -> Self {
        Self { value, span: None }
    }

    /// 就地转换值，保留 span。
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Spanned<U> {
        Spanned {
            value: f(self.value),
            span: self.span,
        }
    }

    pub fn as_ref(&self) -> Spanned<&T> {
        Spanned {
            value: &self.value,
            span: self.span,
        }
    }
}
