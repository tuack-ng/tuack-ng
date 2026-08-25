use crate::{
    doc::rules::{CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule},
    prelude::*,
};
use tuack_ng_parser::ast::block::BlockKind;
use tuack_ng_parser::ast::inline::InlineKind;
use tuack_ng_parser::visitor::{VisitWith, Visitor};

struct HtmlVisitor {
    messages: Vec<CheckInfo>,
}

impl Visitor for HtmlVisitor {
    fn visit_inline(&mut self, inline: &tuack_ng_parser::Inline) {
        if let InlineKind::Html(content) = &inline.value {
            // 忽略 HTML 注释
            if !content.trim_start().starts_with("<!--") {
                self.messages.push(CheckInfo::new(
                    inline.span,
                    format!("检测到内嵌 Html: {}", content),
                    CheckImportance::Warn,
                ));
            }
        }
        self.walk_inline(inline);
    }
    fn visit_block(&mut self, block: &tuack_ng_parser::Block) {
        if let BlockKind::HtmlBlock(content) = &block.value {
            // 忽略 HTML 注释块
            if !content.trim_start().starts_with("<!--") {
                self.messages.push(CheckInfo::new(
                    block.span,
                    format!(
                        "检测到 HTML 块，第一行为：{}",
                        content.lines().nth(0).unwrap_or("")
                    ),
                    CheckImportance::Warn,
                ));
            }
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

    fn check_markdown(&self, _: &str, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }

    fn check_ast(
        &self,
        doc: &tuack_ng_parser::ast::Document,
        _source: &str,
        _problem_config: &ProblemConfig,
    ) -> Result<CheckResult> {
        let mut visitor = HtmlVisitor {
            messages: Vec::new(),
        };
        doc.visit_with(&mut visitor);
        Ok(CheckResult::Tagged(visitor.messages))
    }
}
