use crate::{
    doc::rules::{CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule},
    prelude::*,
};
use lazy_static::lazy_static;
use regex::Regex;
use tuack_ng_parser::ast::Document;
use tuack_ng_parser::span::Span;

lazy_static! {
    static ref SAMPLE_REF_PATTERN: Regex =
        Regex::new(r"\{\{\s*sample\.(text|file)\((\d+)\)\s*\}\}").unwrap();
}

pub struct SamplesNotFound;

impl CheckRule for SamplesNotFound {
    fn manifest(&self) -> CheckManifest {
        CheckManifest {
            name: "sample-not-found".to_string(),
            description: "检查 sample.text/file 对应的文件是否存在以及 ID 是否有效".to_string(),
            markdown_checker: true,
            ast_checker: false,
        }
    }

    fn check_markdown(
        &self,
        markdown_text: &str,
        problem_config: &ProblemConfig,
    ) -> Result<CheckResult> {
        let mut messages: Vec<CheckInfo> = vec![];

        // 查找文档中所有的 sample.text / sample.file
        for caps in SAMPLE_REF_PATTERN.captures_iter(markdown_text) {
            let full_match = caps.get(0).unwrap();
            let kind = caps.get(1).unwrap().as_str();
            let id = caps.get(2).unwrap().as_str().parse::<u32>().unwrap_or(0);
            let span = Span::new(full_match.start(), full_match.end());

            // 查找对应的样本配置
            let sample = match problem_config.samples.iter().find(|s| s.id == id) {
                Some(s) => s,
                None => {
                    messages.push(CheckInfo::new(
                        Some(span),
                        format!("sample.{}({}) 对应的样本配置不存在，ID 无效", kind, id),
                        CheckImportance::Error,
                    ));
                    continue;
                }
            };

            // 只有 sample.text 会渲染文件内容，需要检查文件是否存在
            if kind != "text" {
                continue;
            }

            let mut missing_files = Vec::new();

            // 检查输入文件
            let input_path = sample.input_path();

            let path = problem_config.path.join("sample").join(&input_path);
            if !path.exists() {
                missing_files.push(format!("输入文件 {}", input_path));
            }

            // 检查输出文件
            let output_path = sample.output_path();
            let path = problem_config.path.join("sample").join(&output_path);
            if !path.exists() {
                missing_files.push(format!("输出文件 {}", output_path));
            }

            if !missing_files.is_empty() {
                messages.push(CheckInfo::new(
                    Some(span),
                    format!("sample.text({}) 的 {} 不存在", id, missing_files.join("和")),
                    CheckImportance::Warn,
                ));
            }
        }

        Ok(CheckResult::Tagged(messages))
    }

    fn check_ast(&self, _: &Document, _source: &str, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }
}
