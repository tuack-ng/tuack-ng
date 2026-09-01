//! rushdown AST → 自建 AST 的转换器。

use std::collections::VecDeque;

use rushdown::ast::{Arena, KindData, NodeRef};

use crate::ast::{
    Autolink, Block, BlockKind, CodeBlock, CodeBlockKind, Container, Document, Heading,
    HeadingKind, Image, ImageAttributes, InlineKind, Link, LinkReference, LinkReferenceKind, List,
    ListBulletKind, ListItemKind, ListKind, SetextHeading, Table, TableCell, TableCellKind,
};
use crate::span::{Span, Spanned};

/// 将 rushdown 解析出的 AST 转换为自建结构。
pub(crate) fn convert(arena: &Arena, doc_ref: NodeRef, source: &str) -> Document {
    let mut ctx = Ctx {
        arena,
        source,
        blocks: Vec::new(),
    };
    ctx.convert_children(doc_ref);
    Document {
        blocks: std::mem::take(&mut ctx.blocks),
    }
}

struct Ctx<'a> {
    arena: &'a Arena,
    source: &'a str,
    blocks: Vec<Block>,
}

impl<'a> Ctx<'a> {
    /// 转换某个节点的直接子块节点（用于 document / blockquote / container）。
    fn convert_children(&mut self, node: NodeRef) {
        let children: Vec<NodeRef> = self.arena[node].children(self.arena).collect();
        for child in children {
            let mut inner = Ctx {
                arena: self.arena,
                source: self.source,
                blocks: Vec::new(),
            };
            inner.convert_block(child);
            self.blocks.append(&mut inner.blocks);
        }
    }

    fn convert_block(&mut self, node: NodeRef) {
        let kind = self.arena[node].kind_data();
        match kind {
            KindData::Paragraph(_) => {
                let inlines = self.convert_inline_children(node);
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::Paragraph(inlines),
                    span,
                });
            }
            KindData::Heading(h) => {
                let inlines = self.convert_inline_children(node);
                let kind = match (h.heading_kind(), h.level()) {
                    (rushdown::ast::HeadingKind::Atx, level) => HeadingKind::Atx(level),
                    (rushdown::ast::HeadingKind::Setext, 1) => {
                        HeadingKind::Setext(SetextHeading::Level1)
                    }
                    _ => HeadingKind::Setext(SetextHeading::Level2),
                };
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::Heading(Heading {
                        kind,
                        content: inlines,
                    }),
                    span,
                });
            }
            KindData::ThematicBreak(_) => {
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::ThematicBreak,
                    span,
                });
            }
            KindData::Blockquote(_) => {
                let mut inner = Ctx {
                    arena: self.arena,
                    source: self.source,
                    blocks: Vec::new(),
                };
                inner.convert_children(node);
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::BlockQuote(inner.blocks),
                    span,
                });
            }
            KindData::List(list) => {
                let list_kind = if list.is_ordered() {
                    ListKind::Ordered
                } else {
                    ListKind::Bullet(match list.marker() {
                        b'*' => ListBulletKind::Star,
                        b'+' => ListBulletKind::Plus,
                        b'-' => ListBulletKind::Dash,
                        _ => ListBulletKind::Dash,
                    })
                };

                let items = self.convert_list_items(node);
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::List(List {
                        kind: list_kind,
                        items,
                    }),
                    span,
                });
            }
            KindData::CodeBlock(cb) => {
                let kind = match cb.code_block_kind() {
                    rushdown::ast::CodeBlockKind::Indented => CodeBlockKind::Indented,
                    _ => CodeBlockKind::Fenced {
                        info: cb.info_str(self.source).map(|s| s.to_string()),
                    },
                };
                let literal = cb
                    .value()
                    .iter(self.source)
                    .map(|line| line.into_owned())
                    .collect::<String>();
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::CodeBlock(CodeBlock { kind, literal }),
                    span,
                });
            }
            KindData::HtmlBlock(hb) => {
                let literal = hb
                    .value()
                    .iter(self.source)
                    .map(|line| line.into_owned())
                    .collect::<String>();
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::HtmlBlock(literal),
                    span,
                });
            }
            KindData::LinkReferenceDefinition(def) => {
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::Definition(crate::ast::LinkDefinition {
                        label: def.label_str(self.source).into_owned(),
                        destination: def.destination_str(self.source).to_string(),
                        title: def.title_str(self.source).map(|t| t.into_owned()),
                    }),
                    span,
                });
            }
            KindData::Table(_) => {
                let table = self.convert_table(node);
                let span = self.span_of(node);
                self.blocks.push(Spanned {
                    value: BlockKind::Table(table),
                    span,
                });
            }
            KindData::Extension(ext) => {
                let kind_name = ext.kind_name();
                // fenced-div 扩展节点。
                if ext.as_any().is::<super::ext::fenced_div::FencedDiv>() {
                    let mut inner = Ctx {
                        arena: self.arena,
                        source: self.source,
                        blocks: Vec::new(),
                    };
                    inner.convert_children(node);
                    let (kind, params) = super::ext::fenced_div::fenced_div_to_container(
                        self.arena[node].attributes(),
                        self.source,
                    );
                    let span = self.span_of(node);
                    self.blocks.push(Spanned {
                        value: BlockKind::Container(Container {
                            kind,
                            params,
                            blocks: inner.blocks,
                        }),
                        span,
                    });
                    return;
                }
                // 块级 LaTeX 扩展节点（`$$...$$` 跨行公式）。
                if let Some(block_latex) = ext
                    .as_any()
                    .downcast_ref::<super::ext::latex::LatexBlockNode>()
                {
                    let content = block_latex.content.clone();
                    let span = Some(Span::new(block_latex.start, block_latex.stop));
                    self.blocks.push(Spanned {
                        value: BlockKind::LatexBlock(content),
                        span,
                    });
                    return;
                }
                // 脚注定义节点（`[^label]: 内容`）。
                if let Some(fn_def) = ext
                    .as_any()
                    .downcast_ref::<super::ext::footnote::FootnoteDefinitionNode>()
                {
                    let mut inner = Ctx {
                        arena: self.arena,
                        source: self.source,
                        blocks: Vec::new(),
                    };
                    inner.convert_children(node);
                    let span = self.span_of(node);
                    self.blocks.push(Spanned {
                        value: BlockKind::FootnoteDefinition(crate::ast::FootnoteDefinition {
                            label: fn_def.label.clone(),
                            blocks: inner.blocks,
                        }),
                        span,
                    });
                    return;
                }
                let _ = kind_name;
                self.blocks.push(Spanned::plain(BlockKind::Empty));
            }
            _ => {
                let _ = kind;
                self.blocks.push(Spanned::plain(BlockKind::Empty));
            }
        }
    }

    fn convert_list_items(&mut self, list: NodeRef) -> Vec<Spanned<ListItemKind>> {
        let mut items = Vec::new();
        let children: Vec<NodeRef> = self.arena[list].children(self.arena).collect();
        for child in children {
            if !matches!(self.arena[child].kind_data(), KindData::ListItem(_)) {
                continue;
            }
            let mut inner = Ctx {
                arena: self.arena,
                source: self.source,
                blocks: Vec::new(),
            };
            inner.convert_children(child);
            items.push(Spanned {
                value: ListItemKind::new(inner.blocks),
                span: None,
            });
        }
        items
    }

    /// 转换一个块节点的直接行内子节点。
    fn convert_inline_children(&mut self, node: NodeRef) -> Vec<Spanned<InlineKind>> {
        let mut inlines = Vec::new();
        let children: Vec<NodeRef> = self.arena[node].children(self.arena).collect();
        for child in children {
            self.convert_inline(child, &mut inlines);
        }
        inlines
    }

    fn convert_inline(&mut self, node: NodeRef, out: &mut Vec<Spanned<InlineKind>>) {
        let kind = self.arena[node].kind_data();
        let span = self.span_of(node);
        match kind {
            KindData::Text(t) => {
                let soft_break_after =
                    t.has_qualifiers(rushdown::ast::TextQualifier::SOFT_LINE_BREAK);
                let hard_break_after =
                    t.has_qualifiers(rushdown::ast::TextQualifier::HARD_LINE_BREAK);
                let is_empty = t.index().is_some_and(|idx| idx.is_empty());
                if !is_empty {
                    if let Some(idx) = t.index() {
                        let text = idx.str(self.source);
                        out.push(Spanned {
                            value: InlineKind::Text(text.to_string()),
                            span: Some(Span::new(idx.start(), idx.stop())),
                        });
                    } else {
                        out.push(Spanned::plain(InlineKind::Text(
                            t.str(self.source).to_string(),
                        )));
                    }
                }
                // 软换行：标记在换行前的 Text 上，其后应插入软换行节点。
                if soft_break_after {
                    out.push(Spanned::plain(InlineKind::SoftBreak));
                }
                // 硬换行：行尾 `\` 或两个空格触发，其后应插入硬换行节点。
                if hard_break_after {
                    out.push(Spanned::plain(InlineKind::LineBreak));
                }
            }
            KindData::CodeSpan(cs) => {
                let code = cs.str(self.source).into_owned();
                out.push(Spanned {
                    value: InlineKind::Code(code),
                    span,
                });
            }
            KindData::RawHtml(rh) => {
                let html = rh.str(self.source).into_owned();
                // rushdown 的 RawHtml value 是 Indices，精确覆盖 `<...>`（含尖括号）。
                let html_span = match rh.value() {
                    rushdown::text::MultilineValue::Indices(idxs) => {
                        match (idxs.first(), idxs.last()) {
                            (Some(first), Some(last)) => {
                                Some(Span::new(first.start(), last.stop()))
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                out.push(Spanned {
                    value: InlineKind::Html(html),
                    span: html_span.or(span),
                });
            }
            KindData::Emphasis(_) => {
                let children = self.convert_inline_children(node);
                out.push(Spanned {
                    value: InlineKind::Emphasis(children),
                    span: None,
                });
            }
            KindData::Strong(_) => {
                let children = self.convert_inline_children(node);
                out.push(Spanned {
                    value: InlineKind::Strong(children),
                    span: None,
                });
            }
            KindData::Strikethrough(_) => {
                let children = self.convert_inline_children(node);
                out.push(Spanned {
                    value: InlineKind::Strikethrough(children),
                    span: None,
                });
            }
            KindData::Link(link) => {
                let children = self.convert_inline_children(node);
                let destination = link.destination_str(self.source).to_string();
                let title = link.title_str(self.source).map(|t| t.into_owned());
                // 按 link_kind 分转成三种链接 variant。
                match link.link_kind() {
                    rushdown::ast::LinkKind::Inline => {
                        out.push(Spanned::plain(InlineKind::Link(Link {
                            destination,
                            title,
                            children,
                        })));
                    }
                    rushdown::ast::LinkKind::Reference(r) => {
                        let label = r.value_str(self.source).into_owned();
                        let kind = match r.link_reference_kind() {
                            rushdown::ast::LinkReferenceKind::Full => LinkReferenceKind::Full,
                            rushdown::ast::LinkReferenceKind::Collapsed => {
                                LinkReferenceKind::Collapsed
                            }
                            _ => LinkReferenceKind::Shortcut,
                        };
                        out.push(Spanned::plain(InlineKind::LinkReference(LinkReference {
                            destination,
                            title,
                            label,
                            text: children,
                            kind,
                        })));
                    }
                    rushdown::ast::LinkKind::Auto(a) => {
                        let text = a.text_str(self.source).to_string();
                        out.push(Spanned::plain(InlineKind::Autolink(Autolink {
                            url: destination,
                            text,
                        })));
                    }
                    _ => {
                        out.push(Spanned::plain(InlineKind::Link(Link {
                            destination,
                            title,
                            children,
                        })));
                    }
                }
            }
            KindData::Image(img) => {
                let alt = self.inline_text_plain(node);
                let destination = img.destination_str(self.source).to_string();
                let title = img.title_str(self.source).map(|t| t.into_owned());
                // 属性由 link-attribute 扩展（LinkAttrNode）挂载，此处不设置。
                out.push(Spanned::plain(InlineKind::Image(Image {
                    destination,
                    title,
                    alt,
                    attr: None,
                })));
            }
            KindData::Extension(ext) => {
                if let Some(attrs) = ext
                    .as_any()
                    .downcast_ref::<super::ext::link_attribute::LinkAttrNode>()
                {
                    // 属性挂到前一个 link/image 节点上。
                    let width = attrs.attrs.get("width").cloned();
                    let height = attrs.attrs.get("height").cloned();
                    if let Some(last) = out.last_mut() {
                        if let InlineKind::Image(ref mut image) = last.value {
                            image.attr = Some(ImageAttributes { width, height });
                            return;
                        }
                        if let InlineKind::Link(ref mut _link) = last.value {
                            // link-attribute 属性暂只支持图片，链接属性忽略。
                            return;
                        }
                    }
                    return;
                }
                if let Some(latex) = ext.as_any().downcast_ref::<super::ext::latex::LatexNode>() {
                    // 行内解析到的所有 latex（含 `$$..$$`）都视为行内。
                    // span 由扩展在 parse 时精确记录（覆盖 `$...$`）。
                    out.push(Spanned {
                        value: InlineKind::Latex(latex.content.clone()),
                        span: Some(Span::new(latex.start, latex.stop)),
                    });
                    return;
                }
                if let Some(fn_ref) = ext
                    .as_any()
                    .downcast_ref::<super::ext::footnote::FootnoteReferenceNode>()
                {
                    out.push(Spanned {
                        value: InlineKind::FootnoteReference(fn_ref.label.clone()),
                        span: Some(Span::new(fn_ref.start, fn_ref.stop)),
                    });
                    return;
                }
                let _ = ext;
            }
            _ => {
                let _ = kind;
            }
        }
    }

    /// 提取图片的 alt 文本（子节点的纯文本拼接）。
    fn inline_text_plain(&self, node: NodeRef) -> String {
        let mut s = String::new();
        let children: Vec<NodeRef> = self.arena[node].children(self.arena).collect();
        for child in children {
            match self.arena[child].kind_data() {
                KindData::Text(t) => s.push_str(t.str(self.source)),
                KindData::CodeSpan(cs) => s.push_str(&cs.str(self.source)),
                KindData::Emphasis(_) | KindData::Strong(_) | KindData::Strikethrough(_) => {
                    s.push_str(&self.inline_text_plain(child));
                }
                KindData::RawHtml(rh) => s.push_str(&rh.str(self.source)),
                _ => {}
            }
        }
        s
    }

    /// 转换表格。
    fn convert_table(&mut self, node: NodeRef) -> Table {
        let children: Vec<NodeRef> = self.arena[node].children(self.arena).collect();
        let mut alignments = Vec::new();
        let mut rows: Vec<Vec<TableCell>> = Vec::new();
        let mut col_count = 0usize;

        for child in children {
            let is_header = matches!(self.arena[child].kind_data(), KindData::TableHeader(_));
            let rows_in = match self.arena[child].kind_data() {
                KindData::TableHeader(_) | KindData::TableBody(_) => {
                    let inner: Vec<NodeRef> = self.arena[child].children(self.arena).collect();
                    inner
                }
                _ => Vec::new(),
            };
            for tr in rows_in {
                if matches!(self.arena[tr].kind_data(), KindData::TableRow(_)) {
                    let (row, row_aligns) = self.convert_table_row(tr, false);
                    col_count = col_count.max(row.len());
                    if is_header {
                        alignments = row_aligns;
                    }
                    rows.push(row);
                }
            }
        }

        // 合并逻辑。
        let mut all_rows = rows;
        super::table::process_spans(&mut all_rows);
        Table {
            rows: all_rows,
            alignments,
        }
    }

    fn convert_table_row(
        &mut self,
        row: NodeRef,
        _is_header: bool,
    ) -> (Vec<TableCell>, Vec<crate::ast::Alignment>) {
        let children: Vec<NodeRef> = self.arena[row].children(self.arena).collect();
        let mut cells = Vec::new();
        let mut aligns = Vec::new();
        for cell_node in children {
            if !matches!(self.arena[cell_node].kind_data(), KindData::TableCell(_)) {
                continue;
            }
            // 从 rushdown TableCell 提取对齐。
            let align = match self.arena[cell_node].kind_data() {
                KindData::TableCell(tc) => super::table::alignment_from_rushdown(tc.alignment()),
                _ => crate::ast::Alignment::None,
            };
            aligns.push(align);

            // 单元格精确 span：rushdown 已把 trim 后的内容区间存进 Block source（col_seg）。
            let span = self.table_cell_span(cell_node);
            let mut inlines = Vec::new();
            let cell_children: Vec<NodeRef> = self.arena[cell_node].children(self.arena).collect();
            for c in cell_children {
                self.convert_inline(c, &mut inlines);
            }
            cells.push(Spanned {
                value: TableCellKind::new(inlines),
                span,
            });
        }
        (cells, aligns)
    }

    /// 从 TableCell 节点的 Block source 取精确内容区间。
    ///
    /// rushdown 解析表格时把 trim 后的单元格内容存为 Block source（col_seg），
    /// 精确覆盖内容本身（不含 `| ` 分隔符）。空单元格 source 为空 → None。
    fn table_cell_span(&self, cell_node: NodeRef) -> Option<Span> {
        // 通过节点 type_data 的 Block 访问 source（列单元格内容区间）。
        let type_data = self.arena[cell_node].type_data();
        let block = match type_data {
            rushdown::ast::TypeData::Block(b) => b,
            _ => return None,
        };
        block
            .source()
            .first()
            .map(|seg| Span::new(seg.start(), seg.stop()))
    }

    /// 计算节点 span。
    fn span_of(&self, node: NodeRef) -> Option<Span> {
        let start = self.arena[node].pos()?;
        // 从最后一个后代推算 end。遍历所有直接子节点及其子树。
        let mut end = start;
        let mut stack: VecDeque<NodeRef> = VecDeque::new();
        let children: Vec<NodeRef> = self.arena[node].children(self.arena).collect();
        for c in children {
            stack.push_back(c);
        }
        while let Some(n) = stack.pop_front() {
            if let Some(p) = self.arena[n].pos() {
                end = end.max(p);
            }
            // Text 的 index stop 更精确。
            if let KindData::Text(t) = self.arena[n].kind_data() {
                if let Some(idx) = t.index() {
                    end = end.max(idx.stop());
                }
            }
            let children: Vec<NodeRef> = self.arena[n].children(self.arena).collect();
            for c in children {
                stack.push_back(c);
            }
        }
        if end >= start {
            Some(Span::new(start, end))
        } else {
            None
        }
    }
}
