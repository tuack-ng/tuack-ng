use crate::{
    doc::rules::{CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule},
    prelude::*,
};
use markdown_ppp::{
    ast::*,
    ast_transform::{VisitWith, Visitor},
};

struct HtmlVisitor {
    messages: Vec<CheckInfo>,
}

impl Visitor for HtmlVisitor {
    fn visit_inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Html(content) => {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!("检测到内嵌 Html: {}", content),
                    importance: CheckImportance::Warn,
                });
            }
            _ => {}
        }
        self.walk_inline(inline);
    }
    fn visit_block(&mut self, block: &Block) {
        match block {
            Block::HtmlBlock(content) => {
                self.messages.push(CheckInfo {
                    line: None,
                    col: None,
                    info: format!(
                        "检测到 Html 块, 第一行为: {}",
                        content.lines().nth(0).unwrap_or("")
                    ),
                    importance: CheckImportance::Warn,
                });
            }
            _ => {}
        }
        self.walk_block(block);
    }
}

pub struct Html;

impl CheckRule for Html {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "html".to_string(),
            description: "检测不应出现的 Html".to_string(),
            markdown_checker: false,
            ast_checker: true,
        }
    }

    fn check_markdown(&self, _: &String, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }

    fn check_ast(&self, doc: &Document, _problem_config: &ProblemConfig) -> Result<CheckResult> {
        let mut visitor = HtmlVisitor {
            messages: Vec::new(),
        };
        doc.visit_with(&mut visitor);
        Ok(CheckResult::Tagged(visitor.messages))
    }
}
