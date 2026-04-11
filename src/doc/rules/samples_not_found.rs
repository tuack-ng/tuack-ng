use crate::{
    prelude::*,
    tuack_lib::doc::rules::{CheckImportance, CheckInfo, CheckManifest, CheckResult, CheckRule},
};
use lazy_static::lazy_static;
use markdown_ppp::ast::*;
use regex::Regex;

lazy_static! {
    static ref SAMPLE_TEXT_PATTERN: Regex =
        Regex::new(r"\{\{\s*sample\.text\((\d+)\)\s*\}\}").unwrap();
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

        // 查找文档中所有的 sample.text
        for caps in SAMPLE_TEXT_PATTERN.captures_iter(markdown_text) {
            let full_match = caps.get(0).unwrap();
            let id = caps.get(1).unwrap().as_str().parse::<u32>().unwrap_or(0);
            let col_num = full_match.start();

            // 查找对应的样本配置
            let sample = match problem_config.samples.iter().find(|s| s.id == id) {
                Some(s) => s,
                None => {
                    messages.push(CheckInfo {
                        line: None,
                        col: Some(col_num + 1),
                        info: format!("sample.text({}) 对应的样本配置不存在，ID 无效", id),
                        importance: CheckImportance::Error,
                    });
                    continue;
                }
            };

            let mut missing_files = Vec::new();

            // 检查输入文件
            if let Some(input_path) = sample.input.get() {
                let path = problem_config.path.join("sample").join(input_path);
                if !path.exists() {
                    missing_files.push(format!("输入文件 {}", input_path));
                }
            }

            // 检查输出文件
            if let Some(output_path) = sample.output.get() {
                let path = problem_config.path.join("sample").join(output_path);
                if !path.exists() {
                    missing_files.push(format!("输出文件 {}", output_path));
                }
            }

            if !missing_files.is_empty() {
                messages.push(CheckInfo {
                    line: None,
                    col: Some(col_num + 1),
                    info: format!("sample.text({}) 的 {} 不存在", id, missing_files.join("和")),
                    importance: CheckImportance::Warn,
                });
            }
        }

        Ok(CheckResult::Tagged(messages))
    }

    fn check_ast(&self, _: &Document, _: &ProblemConfig) -> Result<CheckResult> {
        unreachable!()
    }
}
