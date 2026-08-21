use crate::{
    doc::rules::{
        CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule, FormatManifest,
        FormatRule, RuleFile,
    },
    prelude::*,
};
use regex::Regex;
use std::sync::OnceLock;
use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::ast::{Block, Document, Inline};
use tuack_ng_parser::span::{Span, Spanned};

fn input_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"样例\s*(\d+)?\s*输入\s*#?(\d+)?").unwrap())
}

fn output_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"样例\s*(\d+)?\s*输出\s*#?(\d+)?").unwrap())
}

fn extract_text(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|inline| match &inline.value {
            InlineKind::Text(t) => t.clone(),
            InlineKind::Strong(inner) => extract_text(inner),
            InlineKind::Emphasis(inner) => extract_text(inner),
            InlineKind::Strikethrough(inner) => extract_text(inner),
            InlineKind::Code(t) => t.clone(),
            InlineKind::Link(link) => extract_text(&link.children),
            _ => String::new(),
        })
        .collect()
}

enum SampleHeading {
    Input(Option<usize>),
    Output,
    None,
}

fn classify_inlines(inlines: &[Inline]) -> SampleHeading {
    let text = extract_text(inlines).replace(['【', '】'], "");

    if let Some(cap) = input_regex().captures(&text) {
        let n = cap
            .get(1)
            .or_else(|| cap.get(2))
            .and_then(|m| m.as_str().parse().ok());
        return SampleHeading::Input(n);
    }
    if output_regex().captures(&text).is_some() {
        return SampleHeading::Output;
    }
    SampleHeading::None
}

fn classify_block(block: &Block) -> SampleHeading {
    match &block.value {
        BlockKind::Heading(h) => classify_inlines(&h.content),
        BlockKind::Paragraph(inlines) => {
            let is_decorated = inlines.len() == 1
                && matches!(
                    &inlines[0].value,
                    InlineKind::Strong(_) | InlineKind::Emphasis(_)
                );
            let is_plain = inlines
                .iter()
                .all(|i| matches!(i.value, InlineKind::Text(_)));
            if is_decorated || is_plain {
                classify_inlines(inlines)
            } else {
                SampleHeading::None
            }
        }
        _ => SampleHeading::None,
    }
}

pub struct ExportedSample {
    input: String,
    output: String,
    sample_item: SampleItem,
    span: Option<Span>,
}

pub struct SamplesShouldBeExternal;

impl SamplesShouldBeExternal {
    fn format(
        &self,
        doc: Document,
        problem_config: &ProblemConfig,
    ) -> Result<(Document, Vec<ExportedSample>)> {
        let mut new_blocks: Vec<Block> = Vec::new();
        let mut queue: Vec<Block> = Vec::new();
        let mut auto_index = problem_config
            .samples
            .iter()
            .map(|item| item.id)
            .max()
            .unwrap_or(0) as usize
            + 1;

        let mut samples: Vec<ExportedSample> = Vec::new();

        for block in doc.blocks {
            let expected = match queue.len() {
                0 => matches!(classify_block(&block), SampleHeading::Input(_)),
                1 => matches!(&block.value, BlockKind::CodeBlock(_)),
                2 => matches!(classify_block(&block), SampleHeading::Output),
                3 => matches!(&block.value, BlockKind::CodeBlock(_)),
                _ => unreachable!(),
            };

            if expected {
                queue.push(block);
            } else {
                new_blocks.append(&mut queue);
                if matches!(classify_block(&block), SampleHeading::Input(_)) {
                    queue.push(block);
                } else {
                    new_blocks.push(block);
                }
            }

            if queue.len() == 4 {
                let index = match classify_block(&queue[0]) {
                    SampleHeading::Input(n) => n.unwrap_or_else(|| {
                        let i = auto_index;
                        auto_index += 1;
                        i
                    }),
                    _ => unreachable!(),
                };

                debug!("找到应该被提取的样例，id: {}", index);

                let input_code = match &queue[1].value {
                    BlockKind::CodeBlock(cb) => cb.literal.clone(),
                    _ => unreachable!(),
                };
                let output_code = match &queue[3].value {
                    BlockKind::CodeBlock(cb) => cb.literal.clone(),
                    _ => unreachable!(),
                };

                samples.push(ExportedSample {
                    input: input_code,
                    output: output_code,
                    sample_item: SampleItem {
                        id: index as u32,
                        input: None,
                        output: None,
                        args: IndexMap::new(),
                        dmk: None,
                    },
                    // 定位到样例的第一个块（标题块）起始位置。
                    span: queue[0].span,
                });

                new_blocks.push(Spanned::plain(BlockKind::Paragraph(vec![Spanned::plain(
                    InlineKind::Text(format!("{{{{ sample.text({}) }}}}", index)),
                )])));

                queue.clear();
            }
        }

        new_blocks.extend(queue);
        Ok((Document { blocks: new_blocks }, samples))
    }
}

impl FormatRule for SamplesShouldBeExternal {
    fn manifest(&self) -> FormatManifest {
        FormatManifest {
            name: "samples-should-be-external".to_string(),
            description: "应当将样例数据外置到文件中并导入".to_string(),
            markdown_formatter: false,
            ast_formatter: true,
        }
    }

    fn apply_markdown(
        &self,
        _: String,
        _: ProblemConfig,
    ) -> Result<(String, ProblemConfig, Vec<RuleFile>)> {
        unreachable!()
    }

    fn apply_ast(
        &self,
        doc: Document,
        mut problem_config: ProblemConfig,
    ) -> Result<(Document, ProblemConfig, Vec<RuleFile>)> {
        let result = self.format(doc, &problem_config)?;

        let mut files = Vec::new();
        for item in result.1 {
            let index = item.sample_item.id as usize;

            problem_config.samples.push(SampleItem {
                id: index as u32,
                ..item.sample_item
            });

            files.push(RuleFile {
                path: PathBuf::from(format!("sample/{}.in", index)),
                content: item.input.into_bytes(),
            });
            files.push(RuleFile {
                path: PathBuf::from(format!("sample/{}.ans", index)),
                content: item.output.into_bytes(),
            });
        }

        Ok((result.0, problem_config, files))
    }
}

impl CheckRule for SamplesShouldBeExternal {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "samples-should-be-external".to_string(),
            description: "应当将样例数据外置到文件中并导入".to_string(),
            markdown_checker: false,
            ast_checker: true,
        }
    }

    fn check_markdown(&self, _: &str, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }

    fn check_ast(
        &self,
        doc: &Document,
        source: &str,
        problem_config: &ProblemConfig,
    ) -> Result<CheckResult> {
        let result = self.format(doc.to_owned(), problem_config)?;

        let mut messages: Vec<CheckInfo> = vec![];

        for item in result.1 {
            let index = item.sample_item.id as usize;
            let (line, col) = crate::doc::span::span_to_line_col(source, item.span);
            messages.push(CheckInfo {
                line,
                col,
                info: format!("ID 为 {} 的样例内置在了题目内", index),
                importance: CheckImportance::Error,
            });
        }
        Ok(CheckResult::Tagged(messages))
    }
}
