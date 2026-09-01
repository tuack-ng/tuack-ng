//! Markdown 渲染器。
//!
//! 表格**不处理合并**（`<`/`^` 原样输出）。

use crate::ast::block::{BlockKind, CodeBlockKind, ContainerParam, HeadingKind, SetextHeading};
use crate::ast::inline::{InlineKind, LinkReferenceKind};
use crate::ast::list::{ListBulletKind, ListKind};
use crate::ast::{Document, Inline};
use rushdown::util::{is_punct, is_space};

/// 渲染为 Markdown 字符串。
pub fn render_markdown(doc: &Document) -> String {
    let mut out = String::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        render_block(&block.value, &mut out, 0);
    }
    out.push('\n');
    out
}

fn render_block(block: &BlockKind, out: &mut String, _indent: usize) {
    match block {
        BlockKind::Paragraph(inlines) => {
            render_inlines(inlines, out);
        }
        BlockKind::Heading(h) => {
            let (level, setext) = match h.kind {
                HeadingKind::Atx(level) => (level, None),
                // setext 下划线固定 10 个字符。
                HeadingKind::Setext(SetextHeading::Level1) => (0, Some("==========")),
                HeadingKind::Setext(SetextHeading::Level2) => (0, Some("----------")),
            };
            if let Some(underline) = setext {
                render_inlines(&h.content, out);
                out.push('\n');
                out.push_str(underline);
            } else {
                for _ in 0..level {
                    out.push('#');
                }
                out.push(' ');
                render_inlines(&h.content, out);
            }
        }
        BlockKind::ThematicBreak => out.push_str("---"),
        BlockKind::BlockQuote(blocks) => {
            let mut inner = String::new();
            for (i, b) in blocks.iter().enumerate() {
                if i > 0 {
                    inner.push('\n');
                }
                render_block(&b.value, &mut inner, 0);
            }
            // 对每一行加 `> ` 前缀；嵌套引用内部已含前缀，叠加即可。
            let mut first = true;
            for line in inner.lines() {
                if !first {
                    out.push('\n');
                }
                first = false;
                out.push_str("> ");
                out.push_str(line);
            }
        }
        BlockKind::List(list) => {
            let mut counter = 1;
            for (i, item) in list.items.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                let marker = match &list.kind {
                    ListKind::Ordered => {
                        let m = format!("{counter}.");
                        counter += 1;
                        m
                    }
                    ListKind::Bullet(ListBulletKind::Dash) => "-".to_string(),
                    ListKind::Bullet(ListBulletKind::Star) => "*".to_string(),
                    ListKind::Bullet(ListBulletKind::Plus) => "+".to_string(),
                };
                out.push_str(&marker);
                out.push(' ');
                // 渲染列表项内容：非首行缩进到 marker 后。
                let item_prefix = " ".repeat(marker.chars().count() + 1);
                let mut first = true;
                for (j, b) in item.value.blocks.iter().enumerate() {
                    if j > 0 {
                        out.push('\n');
                    }
                    let mut inner = String::new();
                    render_block(&b.value, &mut inner, 0);
                    for line in inner.lines() {
                        if first {
                            out.push_str(line);
                            first = false;
                        } else {
                            out.push('\n');
                            out.push_str(&item_prefix);
                            out.push_str(line);
                        }
                    }
                }
            }
        }
        BlockKind::CodeBlock(cb) => match &cb.kind {
            CodeBlockKind::Fenced { info } => {
                out.push_str("```");
                out.push_str(info.as_deref().unwrap_or(""));
                out.push('\n');
                out.push_str(&cb.literal);
                if !cb.literal.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```");
            }
            CodeBlockKind::Indented => {
                for line in cb.literal.lines() {
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
            }
        },
        BlockKind::HtmlBlock(html) => out.push_str(html),
        BlockKind::Definition(def) => {
            out.push('[');
            out.push_str(&def.label);
            out.push_str("]: ");
            out.push_str(&def.destination);
            if let Some(title) = &def.title {
                out.push_str(" \"");
                out.push_str(title);
                out.push('"');
            }
        }
        BlockKind::Table(table) => render_table(table, out),
        BlockKind::FootnoteDefinition(fn_def) => {
            out.push_str("[^");
            out.push_str(&fn_def.label);
            out.push_str("]: ");
            for (i, b) in fn_def.blocks.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                render_block(&b.value, out, 0);
            }
        }
        BlockKind::Container(c) => {
            out.push_str(":::");
            if kind_is_safe(&c.kind) {
                out.push_str(&c.kind);
            } else {
                // kind 含空格/为空/含特殊字符时裸输出会破坏 fence 行，
                // 改用 class 属性形式输出（值经实体转义），保证再解析不炸。
                out.push_str("{class=\"");
                out.push_str(&escape_attr_value(&c.kind));
                out.push_str("\"}");
            }
            if !c.params.is_empty() {
                // Flag 渲染为裸 `key`（如 `:::align{right}`），KeyValue 输出 `key="value"`。
                let params = c
                    .params
                    .iter()
                    .map(|p| match p {
                        ContainerParam::Flag(k) => k.clone(),
                        ContainerParam::KeyValue(k, v) => {
                            format!("{k}=\"{}\"", escape_attr_value(v))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                out.push_str(&format!("{{{params}}}"));
            }
            out.push('\n');
            for (i, b) in c.blocks.iter().enumerate() {
                if i > 0 {
                    out.push_str("\n\n");
                }
                render_block(&b.value, out, 0);
            }
            out.push_str("\n:::");
        }
        BlockKind::LatexBlock(latex) => {
            out.push_str("$$");
            out.push_str(latex);
            out.push_str("$$");
        }
        BlockKind::Empty => {}
    }
}

/// 判断 kind 能否以裸 `:::kind` 形式输出。
///
/// 与解析端无括号路径（`parse_opening_fence`）的字符集一致：
/// 非空，且每个字节非空格、非标点（`_`/`-`/`:`/`.` 除外）。
fn kind_is_safe(kind: &str) -> bool {
    !kind.is_empty()
        && kind.bytes().all(|b| {
            !is_space(b) && (!is_punct(b) || b == b'_' || b == b'-' || b == b':' || b == b'.')
        })
}

/// 转义属性值中会破坏 fenced-div 语法的字符，保证 `key="value"` 往返幂等。
///
/// 解析端（`parse_attr_value` + `resolve_attr_entities`）会解码 HTML 实体与数字引用，
/// 因此打印端需把 `&`/`"`/换行重编码为实体，否则值里出现这些字符会破坏 `{...}` 属性块。
fn escape_attr_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
    out
}

fn render_inlines(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        render_inline(&inline.value, out);
    }
}

fn render_inline(inline: &InlineKind, out: &mut String) {
    match inline {
        InlineKind::Text(t) => out.push_str(t),
        InlineKind::SoftBreak => out.push('\n'),
        InlineKind::LineBreak => out.push_str("  \n"),
        InlineKind::Code(code) => {
            out.push('`');
            out.push_str(code);
            out.push('`');
        }
        InlineKind::Latex(latex) => {
            out.push('$');
            out.push_str(latex);
            out.push('$');
        }
        InlineKind::Html(html) => out.push_str(html),
        InlineKind::Link(link) => {
            out.push('[');
            render_inlines(&link.children, out);
            out.push_str("](");
            out.push_str(&link.destination);
            if let Some(title) = &link.title {
                out.push_str(" \"");
                out.push_str(title);
                out.push('"');
            }
            out.push(')');
        }
        InlineKind::LinkReference(r) => {
            out.push('[');
            render_inlines(&r.text, out);
            match r.kind {
                LinkReferenceKind::Full => {
                    out.push_str("][");
                    out.push_str(&r.label);
                    out.push(']');
                }
                LinkReferenceKind::Collapsed => out.push_str("][]"),
                LinkReferenceKind::Shortcut => out.push(']'),
            }
        }
        InlineKind::Autolink(a) => {
            out.push('<');
            out.push_str(&a.url);
            out.push('>');
        }
        InlineKind::Image(img) => {
            out.push_str("![");
            out.push_str(&img.alt);
            out.push_str("](");
            out.push_str(&img.destination);
            if let Some(title) = &img.title {
                out.push_str(" \"");
                out.push_str(title);
                out.push('"');
            }
            out.push(')');
            if let Some(attr) = &img.attr {
                let mut parts = Vec::new();
                if let Some(width) = &attr.width {
                    parts.push(format!("width=\"{width}\""));
                }
                if let Some(height) = &attr.height {
                    parts.push(format!("height=\"{height}\""));
                }
                if !parts.is_empty() {
                    out.push_str(&format!(" {{{}}}", parts.join(" ")));
                }
            }
        }
        InlineKind::Emphasis(children) => {
            out.push('*');
            render_inlines(children, out);
            out.push('*');
        }
        InlineKind::Strong(children) => {
            out.push_str("**");
            render_inlines(children, out);
            out.push_str("**");
        }
        InlineKind::Strikethrough(children) => {
            out.push_str("~~");
            render_inlines(children, out);
            out.push_str("~~");
        }
        InlineKind::FootnoteReference(label) => {
            out.push_str("[^");
            out.push_str(label);
            out.push(']');
        }
        InlineKind::Empty => {}
    }
}

/// 渲染表格：纯列对齐，不处理合并。
fn render_table(table: &crate::ast::Table, out: &mut String) {
    if table.rows.is_empty() {
        return;
    }
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| {
                    let mut s = String::new();
                    render_inlines(&cell.value.content, &mut s);
                    s
                })
                .collect()
        })
        .collect();

    let col_count = table
        .alignments
        .len()
        .max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
    let widths = column_widths(&rows, &table.alignments, col_count);

    // header
    out.push('|');
    for (i, cell) in rows[0].iter().enumerate() {
        let align = table
            .alignments
            .get(i)
            .copied()
            .unwrap_or(crate::ast::Alignment::None);
        out.push(' ');
        out.push_str(&align_cell(cell, widths[i], align));
        out.push_str(" |");
    }
    out.push('\n');
    // separator
    out.push('|');
    for (i, align) in table.alignments.iter().enumerate() {
        let w = widths.get(i).copied().unwrap_or(3);
        out.push(' ');
        let sep = match align {
            // ppp：Left 与 None 均输出纯 `-`。
            crate::ast::Alignment::Left | crate::ast::Alignment::None => "-".repeat(w),
            crate::ast::Alignment::Center => format!(":{}:", "-".repeat(w - 2)),
            crate::ast::Alignment::Right => format!("{}:", "-".repeat(w - 1)),
        };
        out.push_str(&sep);
        out.push_str(" |");
    }
    out.push('\n');
    // body
    for row in rows.iter().skip(1) {
        out.push('|');
        for (i, cell) in row.iter().enumerate() {
            let align = table
                .alignments
                .get(i)
                .copied()
                .unwrap_or(crate::ast::Alignment::None);
            out.push(' ');
            out.push_str(&align_cell(
                cell,
                widths.get(i).copied().unwrap_or(3),
                align,
            ));
            out.push_str(" |");
        }
        out.push('\n');
    }
}

/// 计算每列宽度：内容显示宽度与对齐最小宽度取大。
fn column_widths(
    rows: &[Vec<String>],
    alignments: &[crate::ast::Alignment],
    col_count: usize,
) -> Vec<usize> {
    let mut widths = vec![0usize; col_count];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            let w = unicode_width::UnicodeWidthStr::width(cell.as_str());
            if w > widths[i] {
                widths[i] = w;
            }
        }
    }
    for (i, width) in widths.iter_mut().enumerate() {
        let align_min = match alignments.get(i) {
            Some(crate::ast::Alignment::Left) => 1,
            Some(crate::ast::Alignment::Center) => 3,
            Some(crate::ast::Alignment::Right) => 2,
            Some(crate::ast::Alignment::None) | None => 1,
        };
        if *width < align_min {
            *width = align_min;
        }
    }
    widths
}

/// 按对齐方式将内容补齐到列宽（按显示宽度）。
fn align_cell(cell: &str, width: usize, alignment: crate::ast::Alignment) -> String {
    let cell_width = unicode_width::UnicodeWidthStr::width(cell);
    let padding = width.saturating_sub(cell_width);
    match alignment {
        crate::ast::Alignment::None | crate::ast::Alignment::Left => {
            format!("{}{}", cell, " ".repeat(padding))
        }
        crate::ast::Alignment::Center => {
            let left_padding = padding / 2;
            let right_padding = padding - left_padding;
            format!(
                "{}{}{}",
                " ".repeat(left_padding),
                cell,
                " ".repeat(right_padding)
            )
        }
        crate::ast::Alignment::Right => format!("{}{}", " ".repeat(padding), cell),
    }
}
