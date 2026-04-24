use crate::{
    prelude::*,
    tuack_lib::doc::rules::{
        CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule, FormatManifest,
        FormatRule,
    },
};
use markdown_ppp::ast::*;
use regex::Regex;
use std::sync::OnceLock;

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
        .map(|inline| match inline {
            Inline::Text(t) => t.clone(),
            Inline::Strong(inner) => extract_text(inner),
            Inline::Emphasis(inner) => extract_text(inner),
            Inline::Strikethrough(inner) => extract_text(inner),
            Inline::Code(t) => t.clone(),
            Inline::Link(link) => extract_text(&link.children),
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
    // .replace('「', "")
    // .replace('」', "")
    // .replace('《', "")
    // .replace('》', "");

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
    match block {
        Block::Heading(h) => classify_inlines(&h.content),
        Block::Paragraph(inlines) => {
            let is_decorated = inlines.len() == 1
                && matches!(&inlines[0], Inline::Strong(_) | Inline::Emphasis(_));
            let is_plain = inlines.iter().all(|i| matches!(i, Inline::Text(_)));
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
            .unwrap_or(0) as usize;

        let mut samples: Vec<ExportedSample> = Vec::new();

        for block in doc.blocks {
            let expected = match queue.len() {
                0 => matches!(classify_block(&block), SampleHeading::Input(_)),
                1 => matches!(&block, Block::CodeBlock(_)),
                2 => matches!(classify_block(&block), SampleHeading::Output),
                3 => matches!(&block, Block::CodeBlock(_)),
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

                debug!("找到应该被提取的样例, id: {}", index);

                let input_code = match &queue[1] {
                    Block::CodeBlock(cb) => cb.literal.clone(),
                    _ => unreachable!(),
                };
                let output_code = match &queue[3] {
                    Block::CodeBlock(cb) => cb.literal.clone(),
                    _ => unreachable!(),
                };

                samples.push(ExportedSample {
                    input: input_code,
                    output: output_code,
                    sample_item: SampleItem {
                        id: index as u32,
                        input: None,
                        output: None,
                        args: HashMap::new(),
                        dmk: None,
                    },
                });

                new_blocks.push(markdown_ppp::ast::Block::Paragraph(vec![
                    markdown_ppp::ast::Inline::Text(format!("{{{{ sample.text({}) }}}}", index)),
                ]));

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

    fn apply_markdown(&self, _: String, _: ProblemConfig) -> Result<(String, ProblemConfig)> {
        unreachable!()
    }

    fn apply_ast(
        &self,
        doc: Document,
        mut problem_config: ProblemConfig,
    ) -> Result<(Document, ProblemConfig)> {
        let result = self.format(doc, &problem_config)?;

        for item in result.1 {
            let index = item.sample_item.id as usize;
            let sample_path = problem_config.path.join("sample");

            if !sample_path.exists() {
                fs::create_dir(&sample_path)?;
            }

            problem_config.samples.push(SampleItem {
                id: index as u32,
                ..item.sample_item
            });

            fs::write(sample_path.join(format!("{}.in", index)), &item.input)?;
            fs::write(sample_path.join(format!("{}.ans", index)), &item.output)?;
        }

        Ok((result.0, problem_config))
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

    fn check_ast(&self, doc: &Document, problem_config: &ProblemConfig) -> Result<CheckResult> {
        let result = self.format(doc.to_owned(), problem_config)?;

        let mut messages: Vec<CheckInfo> = vec![];

        for item in result.1 {
            let index = item.sample_item.id as usize;
            messages.push(CheckInfo {
                line: None,
                col: None,
                info: format!("ID 为 {} 的样例内置在了题目内", index),
                importance: CheckImportance::Error,
            });
        }
        Ok(CheckResult::Tagged(messages))
    }
}
