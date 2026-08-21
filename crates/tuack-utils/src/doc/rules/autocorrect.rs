use crate::{
    doc::rules::{
        CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule, FormatManifest,
        FormatRule, RuleFile,
    },
    prelude::*,
};
use autocorrect::{Severity, format_for, lint_for};
use tuack_ng_parser::ast::Document;

/*
已知在行中有 $$, {{ }} 时，不会告警没有句号，对于 {{ }}，也不会保留空格。
但是由于这两种情况复杂（比如：模板语句可能不需要句号，$$ 可能后面会是 ;；. ；），实际上没有什么妥善方法检查，暂时忽略。
*/

pub struct Autocorrect;

impl FormatRule for Autocorrect {
    fn manifest(&self) -> FormatManifest {
        FormatManifest {
            name: "autocorrect".to_string(),
            description: "纠正中日韩文字与英文之间的标点、空格等问题".to_string(),
            markdown_formatter: true,
            ast_formatter: false,
        }
    }

    fn apply_markdown(
        &self,
        markdown_text: String,
        problem_config: ProblemConfig,
    ) -> Result<(String, ProblemConfig, Vec<RuleFile>)> {
        autocorrect::config::load(
            r#"
            rules:
                space-dollar: 1
            "#,
        )
        .unwrap();
        let format_result = format_for(&markdown_text, "md");
        if format_result.has_error() {
            bail!("格式化失败：{}", format_result.error);
        }
        Ok((format_result.out, problem_config, Vec::new()))
    }

    fn apply_ast(
        &self,
        _: Document,
        _: ProblemConfig,
    ) -> Result<(Document, ProblemConfig, Vec<RuleFile>)> {
        unreachable!()
    }
}

impl CheckRule for Autocorrect {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "autocorrect".to_string(),
            description: "纠正中日韩文字与英文之间的标点、空格等问题".to_string(),
            markdown_checker: true,
            ast_checker: false,
        }
    }

    fn check_markdown(&self, markdown_text: &str, _: &ProblemConfig) -> Result<CheckResult> {
        autocorrect::config::load(
            r#"
            rules:
                space-dollar: 1
            "#,
        )
        .unwrap();
        let check_result = lint_for(markdown_text, "md");

        if check_result.has_error() {
            bail!("检查失败：{}", check_result.error);
        }
        let mut messages: Vec<CheckInfo> = vec![];
        for info in check_result.lines {
            messages.push(CheckInfo {
                line: Some(info.line),
                col: Some(info.col),
                info: format!("原文为 '{}', 应为 '{}'", info.old, info.new),
                importance: match info.severity {
                    Severity::Warning => CheckImportance::Warn,
                    Severity::Error => CheckImportance::Error,
                    _ => unreachable!(),
                },
            })
        }
        Ok(CheckResult::Tagged(messages))
    }

    fn check_ast(&self, _: &Document, _source: &str, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }
}
